// Render benchmark for the reference C++ runtime; mirrors examples/bench.rs exactly.
//
// Usage: render_bench <skeleton.json|.skel> <atlas> <animation> <frames>
// Prints: render_ns=<per frame> bounds_ns=<per frame> bbox_update_ns=<per frame> hit_ns=<per frame>
// hit_count=<total> checksum=<value>
// bbox_update_ns is SkeletonBounds::update; hit_ns is 16 containsPoint calls on a grid over the
// AABB of the bounding boxes (the drawn area when there are none). The answers are part of the
// checksum, so the work cannot be dropped; hit_count is how many points landed in a box.
// Only the render step is timed; posing the skeleton happens outside the timed region.

#include <chrono>
#include <spine/spine.h>
#include <stdio.h>
#include <string.h>

using namespace spine;
namespace spine {
	SpineExtension *getDefaultExtension() { return new DefaultSpineExtension(); }
}
class HeadlessTextureLoader : public TextureLoader {
public:
	virtual void load(AtlasPage &page, const String &path) override { page.texture = (void *) (intptr_t) (page.index + 1); page.width = 1024; page.height = 1024; }
	virtual void unload(void *texture) override {}
};

int main(int argc, char *argv[]) {
	if (argc < 5) return 1;
	const char *path = argv[1], *atlasPath = argv[2], *animationName = argv[3];
	int frames = atoi(argv[4]);
	Bone::setYDown(false);
	HeadlessTextureLoader textureLoader;
	Atlas atlas(atlasPath, &textureLoader);
	SkeletonData *data;
	if (strstr(path, ".json")) { SkeletonJson json(atlas); data = json.readSkeletonDataFile(path); }
	else { SkeletonBinary binary(atlas); data = binary.readSkeletonDataFile(path); }
	Animation *animation = data->findAnimation(animationName);
	if (!data || !animation) return 1;

	Skeleton skeleton(*data);
	AnimationStateData stateData(*data);
	AnimationState state(stateData);
	state.setAnimation(0, *animation, true);
	SkeletonRenderer renderer;
	SkeletonClipping clipper;
	SkeletonBounds bounds;
	Array<float> scratch;
	const float dt = 1 / 60.0f;
	using clock = std::chrono::steady_clock;
	double renderNs = 0, boundsNs = 0, bboxUpdateNs = 0, hitNs = 0, checksum = 0;
	long long hitCount = 0;

	for (int i = -600; i < frames; i++) { // 600 warm-up frames are not timed
		skeleton.update(dt); state.update(dt); state.apply(skeleton); skeleton.updateWorldTransform(Physics_Update);
		auto t0 = clock::now();
		RenderCommand *cmd = renderer.render(skeleton);
		auto t1 = clock::now();
		float x, y, w, h;
		skeleton.getBounds(x, y, w, h, scratch, &clipper);
		auto t2 = clock::now();
		bounds.update(skeleton, true);
		auto t3 = clock::now();
		float gridX = x, gridY = y, gridW = w, gridH = h;
		if (bounds.getPolygons().size() > 0) {
			gridX = bounds.getMinX(); gridY = bounds.getMinY(); gridW = bounds.getWidth(); gridH = bounds.getHeight();
		}
		double hits = 0;
		for (int gi = 0; gi < 4; gi++) {
			for (int gj = 0; gj < 4; gj++) {
				float px = gridX + gridW * ((float) gi + 0.5f) / 4.0f;
				float py = gridY + gridH * ((float) gj + 0.5f) / 4.0f;
				BoundingBoxAttachment *hit = bounds.containsPoint(px, py);
				if (hit) { hits += 1.0 + (double) hit->getName().length(); hitCount++; }
			}
		}
		auto t4 = clock::now();
		if (i < 0) continue;
		renderNs += std::chrono::duration<double, std::nano>(t1 - t0).count();
		boundsNs += std::chrono::duration<double, std::nano>(t2 - t1).count();
		bboxUpdateNs += std::chrono::duration<double, std::nano>(t3 - t2).count();
		hitNs += std::chrono::duration<double, std::nano>(t4 - t3).count();
		for (; cmd; cmd = cmd->next) {
			checksum += cmd->numVertices + cmd->numIndices;
			for (int v = 0; v < cmd->numVertices * 2; v++) checksum += cmd->positions[v] * 0.001;
		}
		checksum += (double) (x + y + w + h) + hits;
	}
	printf("render_ns=%.1f bounds_ns=%.1f bbox_update_ns=%.1f hit_ns=%.1f hit_count=%lld checksum=%.3f\n", renderNs / frames, boundsNs / frames, bboxUpdateNs / frames, hitNs / frames, hitCount, checksum);
	return 0;
}
