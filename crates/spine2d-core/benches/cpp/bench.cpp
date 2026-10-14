// Benchmark for the reference C++ runtime; mirrors examples/bench.rs exactly.
//
// Usage: bench <skeleton.json|.skel> <atlas> <animation> <frames> <loads>
// Prints: load_us=<per load> ctor_us=<per Skeleton> frame_ns=<per frame> checksum=<value>

#include <chrono>
#include <math.h>
#include <spine/spine.h>
#include <stdio.h>
#include <string.h>

using namespace spine;
namespace spine {
	SpineExtension *getDefaultExtension() { return new DefaultSpineExtension(); }
}
class HeadlessTextureLoader : public TextureLoader {
public:
	virtual void load(AtlasPage &page, const String &path) override { page.texture = nullptr; page.width = 1024; page.height = 1024; }
	virtual void unload(void *texture) override {}
};

static SkeletonData *loadData(const char *path, Atlas &atlas) {
	if (strstr(path, ".json")) { SkeletonJson json(atlas); return json.readSkeletonDataFile(path); }
	SkeletonBinary binary(atlas);
	return binary.readSkeletonDataFile(path);
}

int main(int argc, char *argv[]) {
	if (argc < 6) return 1;
	const char *path = argv[1], *atlasPath = argv[2], *animationName = argv[3];
	int frames = atoi(argv[4]), loads = atoi(argv[5]);
	Bone::setYDown(false);
	HeadlessTextureLoader textureLoader;
	Atlas atlas(atlasPath, &textureLoader);
	using clock = std::chrono::steady_clock;

	auto t0 = clock::now();
	for (int i = 0; i < loads; i++) { SkeletonData *d = loadData(path, atlas); delete d; }
	double loadUs = std::chrono::duration<double, std::micro>(clock::now() - t0).count() / loads;

	SkeletonData *data = loadData(path, atlas);
	Animation *animation = data->findAnimation(animationName);
	if (!animation) { fprintf(stderr, "no animation\n"); return 1; }

	t0 = clock::now();
	for (int i = 0; i < loads * 10; i++) { Skeleton s(*data); }
	double ctorUs = std::chrono::duration<double, std::micro>(clock::now() - t0).count() / (loads * 10);

	Skeleton skeleton(*data);
	AnimationStateData stateData(*data);
	AnimationState state(stateData);
	state.setAnimation(0, *animation, true);
	const float dt = 1 / 60.0f;
	for (int i = 0; i < 600; i++) { // warm up
		skeleton.update(dt); state.update(dt); state.apply(skeleton); skeleton.updateWorldTransform(Physics_Update);
	}
	t0 = clock::now();
	for (int i = 0; i < frames; i++) {
		skeleton.update(dt); state.update(dt); state.apply(skeleton); skeleton.updateWorldTransform(Physics_Update);
	}
	double frameNs = std::chrono::duration<double, std::nano>(clock::now() - t0).count() / frames;

	double checksum = 0;
	for (size_t i = 0; i < skeleton.getBones().size(); i++) {
		BonePose &p = skeleton.getBones()[i]->getAppliedPose();
		checksum += (double) p.getWorldX() + 2.0 * (double) p.getWorldY() + 0.5 * (double) p.getA();
	}
	printf("load_us=%.1f ctor_us=%.2f frame_ns=%.1f checksum=%.4f\n", loadUs, ctorUs, frameNs, checksum);
	return 0;
}
