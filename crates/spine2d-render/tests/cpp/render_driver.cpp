// Golden-test driver for spine2d-render: plays an animation with the reference C++ runtime and
// prints, per frame, the SkeletonRenderer commands, Skeleton::getBounds (with and without a
// clipper) and SkeletonBounds queries.
//
// Usage: render_driver <skeleton.json|.skel> <atlas|-> <animation|-> <frames> [loop]

#include <math.h>
#include <spine/spine.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

using namespace spine;

namespace spine {
	SpineExtension *getDefaultExtension() { return new DefaultSpineExtension(); }
}

class HeadlessTextureLoader : public TextureLoader {
public:
	virtual void load(AtlasPage &page, const String &path) override {
		page.texture = (void *) (intptr_t) (page.index + 1);
		page.width = 1024;
		page.height = 1024;
	}
	virtual void unload(void *texture) override {}
};

// Non-finite values (eg the width of empty bounds) are printed as quoted strings to keep the JSON valid.
static void printF(float v) {
	if (isfinite(v)) printf("%.6f", v); else printf("\"%s\"", isnan(v) ? "NaN" : (v < 0 ? "-inf" : "inf"));
}

static void printFloats(const float *v, int n) {
	printf("[");
	for (int i = 0; i < n; i++) { if (i) printf(","); printF(v[i]); }
	printf("]");
}

static void printBounds(float x, float y, float w, float h) {
	printf("{\"x\":"); printF(x); printf(",\"y\":"); printF(y); printf(",\"w\":"); printF(w); printf(",\"h\":"); printF(h); printf("}");
}

static void printHit(BoundingBoxAttachment *a) {
	if (a) printf("\"%s\"", a->getName().buffer()); else printf("null");
}

int main(int argc, char *argv[]) {
	if (argc < 5) return 1;
	const char *skeletonPath = argv[1], *atlasPath = argv[2], *animationName = argv[3];
	int frames = atoi(argv[4]);
	bool loop = argc > 5 && atoi(argv[5]) != 0;
	const float dt = 1 / 60.0f;

	Bone::setYDown(false);
	HeadlessTextureLoader textureLoader;
	Atlas *atlas = strcmp(atlasPath, "-") == 0 ? new Atlas("", 0, "", &textureLoader, false) : new Atlas(atlasPath, &textureLoader);
	SkeletonData *data;
	if (strstr(skeletonPath, ".json")) {
		SkeletonJson json(*atlas);
		data = json.readSkeletonDataFile(skeletonPath);
		if (!data) fprintf(stderr, "%s\n", json.getError().buffer());
	} else {
		SkeletonBinary binary(*atlas);
		data = binary.readSkeletonDataFile(skeletonPath);
		if (!data) fprintf(stderr, "%s\n", binary.getError().buffer());
	}
	if (!data) return 1;

	Skeleton skeleton(*data);
	AnimationStateData stateData(*data);
	AnimationState state(stateData);
	if (strcmp(animationName, "-") != 0) {
		Animation *animation = data->findAnimation(animationName);
		if (!animation) return 1;
		state.setAnimation(0, *animation, loop);
	}

	SkeletonRenderer renderer;
	SkeletonBounds bounds;
	SkeletonClipping clipper;
	for (int i = 0; i < frames; i++) {
		skeleton.setPosition(sinf(i * 0.15f) * 80, cosf(i * 0.11f) * 30);
		skeleton.update(dt);
		state.update(dt);
		state.apply(skeleton);
		skeleton.updateWorldTransform(i == 0 ? Physics_Reset : Physics_Update);

		printf("\n=== ANIMATION FRAME %d ===\n{\"commands\":[", i);
		bool first = true;
		for (RenderCommand *cmd = renderer.render(skeleton); cmd; cmd = cmd->next) {
			if (!first) printf(",");
			first = false;
			printf("{\"blend\":%d,\"texture\":%d,\"positions\":", (int) cmd->blendMode, (int) (intptr_t) cmd->texture);
			printFloats(cmd->positions, cmd->numVertices * 2);
			printf(",\"uvs\":");
			printFloats(cmd->uvs, cmd->numVertices * 2);
			printf(",\"colors\":[");
			for (int v = 0; v < cmd->numVertices; v++) printf(v ? ",%u" : "%u", cmd->colors[v]);
			printf("],\"dark\":[");
			for (int v = 0; v < cmd->numVertices; v++) printf(v ? ",%u" : "%u", cmd->darkColors[v]);
			printf("],\"indices\":[");
			for (int v = 0; v < cmd->numIndices; v++) printf(v ? ",%u" : "%u", cmd->indices[v]);
			printf("]}");
		}
		printf("],\"bounds\":");
		float x, y, w, h;
		skeleton.getBounds(x, y, w, h);
		printBounds(x, y, w, h);
		printf(",\"clippedBounds\":");
		Array<float> buffer;
		skeleton.getBounds(x, y, w, h, buffer, &clipper);
		printBounds(x, y, w, h);

		bounds.update(skeleton, true);
		printf(",\"aabb\":{\"minX\":%.6f,\"minY\":%.6f,\"maxX\":%.6f,\"maxY\":%.6f},\"boxes\":%d", bounds.getMinX(), bounds.getMinY(), bounds.getMaxX(),
			   bounds.getMaxY(), (int) bounds.getBoundingBoxes().size());
		printf(",\"points\":[");
		bool any = bounds.getBoundingBoxes().size() > 0;
		if (any) {
			for (int gx = 0; gx < 5; gx++) {
				for (int gy = 0; gy < 5; gy++) {
					float px = bounds.getMinX() + (bounds.getMaxX() - bounds.getMinX()) * (gx + 0.5f) / 5;
					float py = bounds.getMinY() + (bounds.getMaxY() - bounds.getMinY()) * (gy + 0.5f) / 5;
					if (gx || gy) printf(",");
					printf("{\"in\":%d,", bounds.aabbContainsPoint(px, py) ? 1 : 0);
					printf("\"hit\":");
					printHit(bounds.containsPoint(px, py));
					printf("}");
				}
			}
		}
		printf("],\"segments\":[");
		if (any) {
			float x0 = bounds.getMinX(), y0 = bounds.getMinY(), x1 = bounds.getMaxX(), y1 = bounds.getMaxY();
			float segs[4][4] = {{x0 - 10, y0 - 10, x1 + 10, y1 + 10}, {x0 - 10, y1 + 10, x1 + 10, y0 - 10}, {x0, (y0 + y1) / 2, x1, (y0 + y1) / 2}, {(x0 + x1) / 2, y0 - 50, (x0 + x1) / 2, y1 + 50}};
			for (int s = 0; s < 4; s++) {
				if (s) printf(",");
				printf("{\"aabb\":%d,\"hit\":", bounds.aabbIntersectsSegment(segs[s][0], segs[s][1], segs[s][2], segs[s][3]) ? 1 : 0);
				printHit(bounds.intersectsSegment(segs[s][0], segs[s][1], segs[s][2], segs[s][3]));
				printf("}");
			}
		}
		printf("]}");
	}
	printf("\n");
	return 0;
}
