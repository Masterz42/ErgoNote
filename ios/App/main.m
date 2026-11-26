#import <UIKit/UIKit.h>
#import <objc/runtime.h>

extern void ergonote_ios_main(void);

static __weak UIWindowScene *gActiveScene = nil;

static void attachWindowToActiveScene(UIWindow *window) {
    UIWindowScene *scene = gActiveScene;
    if (window != nil && scene != nil && window.windowScene != scene) {
        window.windowScene = scene;
        window.frame = scene.coordinateSpace.bounds;
    }
}

static void attachAllWindowsToActiveScene(void) {
    UIWindowScene *scene = gActiveScene;
    if (scene == nil) {
        return;
    }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    NSArray<UIWindow *> *windows = UIApplication.sharedApplication.windows;
#pragma clang diagnostic pop
    for (UIWindow *w in windows) {
        attachWindowToActiveScene(w);
    }
}

@implementation UIWindow (ErgoNoteSceneBridge)

+ (void)load {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        Method orig = class_getInstanceMethod(self, @selector(makeKeyAndVisible));
        Method swiz = class_getInstanceMethod(self, @selector(ergonote_makeKeyAndVisible));
        if (orig != NULL && swiz != NULL) {
            method_exchangeImplementations(orig, swiz);
        }
    });
}

- (void)ergonote_makeKeyAndVisible {
    attachWindowToActiveScene(self);
    [self ergonote_makeKeyAndVisible];
}

@end

@interface ErgoNoteSceneDelegate : UIResponder <UIWindowSceneDelegate>
@property (strong, nonatomic) UIWindow *window;
@end

@implementation ErgoNoteSceneDelegate

- (void)scene:(UIScene *)scene
    willConnectToSession:(UISceneSession *)session
                 options:(UISceneConnectionOptions *)connectionOptions {
    (void)session;
    (void)connectionOptions;
    if ([scene isKindOfClass:[UIWindowScene class]]) {
        UIWindowScene *windowScene = (UIWindowScene *)scene;
        gActiveScene = windowScene;
        attachAllWindowsToActiveScene();
        if (self.window == nil && windowScene.windows.firstObject != nil) {
            self.window = windowScene.windows.firstObject;
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            attachAllWindowsToActiveScene();
            if (self.window == nil && windowScene.windows.firstObject != nil) {
                self.window = windowScene.windows.firstObject;
            }
        });
    }
}

- (void)sceneDidDisconnect:(UIScene *)scene {
    if (scene == gActiveScene) {
        gActiveScene = nil;
    }
}

@end

int main(void) {
    @autoreleasepool {
        ergonote_ios_main();
    }
    return 0;
}
