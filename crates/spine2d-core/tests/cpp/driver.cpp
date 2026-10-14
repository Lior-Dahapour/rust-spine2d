// Golden-test driver for spine2d-core: loads a skeleton with the reference C++ runtime, optionally
// plays an animation, advances skeleton time and moves the skeleton (so physics react), and
// prints every frame in the same format as spine-cpp's HeadlessTest.
//
// Usage: driver <skeleton.json|.skel> <atlas|-> <animation|-> <frames> [loop] [dt] [additive] [alpha]
//
// Build (from a spine-runtimes checkout, see tests/golden.rs):
//   clang++ -std=c++11 -O1 -w -ffp-contract=off -I$SRC/include -o driver \
//       tests/cpp/driver.cpp $SRC/src/spine/*.cpp

#include <math.h>
#include <spine/spine.h>
#include <stdio.h>
#include <string.h>

using namespace spine;

namespace spine {
	SpineExtension *getDefaultExtension() {
		return new DefaultSpineExtension();
	}
}// namespace spine

class HeadlessTextureLoader : public TextureLoader {
public:
	virtual void load(AtlasPage &page, const String &path) override {
		page.texture = nullptr;
		page.width = 1024;
		page.height = 1024;
	}
	virtual void unload(void *texture) override {}
};

static void printBonePose(BonePose &pose) {
	printf("{\"x\":%.6f,\"y\":%.6f,\"rotation\":%.6f,\"scaleX\":%.6f,\"scaleY\":%.6f,\"shearX\":%.6f,\"shearY\":%.6f,", pose.getX(), pose.getY(),
		   pose.getRotation(), pose.getScaleX(), pose.getScaleY(), pose.getShearX(), pose.getShearY());
	printf("\"a\":%.6f,\"b\":%.6f,\"c\":%.6f,\"d\":%.6f,\"worldX\":%.6f,\"worldY\":%.6f}", pose.getA(), pose.getB(), pose.getC(), pose.getD(),
		   pose.getWorldX(), pose.getWorldY());
}

static void printFrame(Skeleton &skeleton, int frame) {
	printf("\n=== ANIMATION FRAME %d ===\n{\"frame\":%d,\"bones\":[", frame, frame);
	Array<Bone *> &bones = skeleton.getBones();
	for (size_t i = 0; i < bones.size(); i++) {
		if (i) printf(",");
		printf("{\"name\":\"%s\",\"pose\":", bones[i]->getData().getName().buffer());
		printBonePose(bones[i]->getPose());
		printf(",\"appliedPose\":");
		printBonePose(bones[i]->getAppliedPose());
		printf("}");
	}
	printf("],\"slots\":[");
	Array<Slot *> &slots = skeleton.getSlots();
	for (size_t i = 0; i < slots.size(); i++) {
		if (i) printf(",");
		SlotPose &pose = slots[i]->getPose();
		Attachment *attachment = pose.getAttachment();
		Color &c = pose.getColor();
		printf("{\"name\":\"%s\",\"pose\":{\"attachment\":", slots[i]->getData().getName().buffer());
		if (attachment) printf("\"%s\"", attachment->getName().buffer()); else printf("null");
		printf(",\"sequenceIndex\":%d,\"color\":{\"r\":%.6f,\"g\":%.6f,\"b\":%.6f,\"a\":%.6f}}}", pose.getSequenceIndex(), c.r, c.g, c.b, c.a);
	}
	printf("]}");
}

int main(int argc, char *argv[]) {
	if (argc < 5) {
		fprintf(stderr, "usage: driver <skeleton> <atlas|-> <animation|-> <frames> [loop] [dt] [additive] [alpha]\n");
		return 1;
	}
	const char *skeletonPath = argv[1], *atlasPath = argv[2], *animationName = argv[3];
	int frames = atoi(argv[4]);
	bool loop = argc > 5 && atoi(argv[5]) != 0;
	float dt = argc > 6 ? (float) atof(argv[6]) : 1 / 60.0f;
	bool additive = argc > 7 && atoi(argv[7]) != 0;
	float alpha = argc > 8 ? (float) atof(argv[8]) : 1.0f;

	Bone::setYDown(false);// like HeadlessTest
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
		if (!animation) {
			fprintf(stderr, "animation not found\n");
			return 1;
		}
		TrackEntry &entry = state.setAnimation(0, *animation, loop);
		entry.setAdditive(additive);
		entry.setAlpha(alpha);
	}

	for (int i = 0; i < frames; i++) {
		// Move the skeleton so physics have something to react to.
		skeleton.setPosition(sinf(i * 0.15f) * 80, cosf(i * 0.11f) * 30);
		skeleton.update(dt);
		state.update(dt);
		state.apply(skeleton);
		skeleton.updateWorldTransform(i == 0 ? Physics_Reset : Physics_Update);
		printFrame(skeleton, i);
	}
	printf("\n");
	return 0;
}
