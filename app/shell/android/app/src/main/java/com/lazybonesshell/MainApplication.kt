package com.lazybonesshell

import android.app.Application
import android.content.res.Configuration
import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactHost
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative

// Expo bare-modules integration (SDK 57). ExpoReactHostFactory wraps RN's default host so autolinked
// expo-* modules (expo-secure-store, …) are registered; ApplicationLifecycleDispatcher forwards
// Application lifecycle events those modules subscribe to. Re.Pack + Module Federation are untouched —
// this is native module wiring only. See docs/scope/app/app-expo-scope.md.
import expo.modules.ApplicationLifecycleDispatcher
import expo.modules.ExpoReactHostFactory

class MainApplication : Application(), ReactApplication {

  override val reactHost: ReactHost by lazy {
    ExpoReactHostFactory.getDefaultReactHost(
      context = applicationContext,
      packageList =
        PackageList(this).packages.apply {
          // Packages that cannot be autolinked yet can be added manually here, for example:
          // add(MyReactNativePackage())
        },
      // Re.Pack (not Expo's Metro) is our bundler and it serves the JS entry at `/index.bundle`.
      // Expo's default `jsMainModulePath` is `.expo/.virtual-metro-entry`, so the dev host would
      // fetch `/.expo/.virtual-metro-entry.bundle` → 404 against Re.Pack and the app dies on load.
      // Point it back at our real entry (`index.js`). See docs/scope/app/app-expo-scope.md.
      jsMainModulePath = "index",
    )
  }

  override fun onCreate() {
    super.onCreate()
    loadReactNative(this)
    ApplicationLifecycleDispatcher.onApplicationCreate(this)
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    ApplicationLifecycleDispatcher.onConfigurationChanged(this, newConfig)
  }
}
