#!/bin/bash
# Build LLM Gateway Android APK
set -e

export ANDROID_HOME=/home/oyhx/android-sdk
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64

cd "$(dirname "$0")"

echo "🚀 Building LLM Gateway Android APK..."

if [ "$1" = "release" ]; then
    echo "Building release APK..."
    ./gradlew assembleRelease
    APK_PATH="app/build/outputs/apk/release/app-release-unsigned.apk"
else
    echo "Building debug APK..."
    ./gradlew assembleDebug
    APK_PATH="app/build/outputs/apk/debug/app-debug.apk"
fi

if [ -f "$APK_PATH" ]; then
    SIZE=$(ls -lh "$APK_PATH" | awk '{print $5}')
    echo "✅ APK built successfully: $APK_PATH ($SIZE)"
    echo "   Copy to phone and install with: adb install $APK_PATH"
else
    echo "❌ APK not found at $APK_PATH"
    exit 1
fi
