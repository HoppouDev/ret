plugins {
    id("com.android.application")
}

base {
    archivesName = "ret" // change apk name
}

// to be defined in gradle.properties (or with -P) to sign the release apk
val releaseStoreFile = providers.gradleProperty("RELEASE_STORE_FILE")

android {
    namespace = "dev.hoppou.ret"
    compileSdk {
        version = release(36) {
            minorApiLevel = 1
        }
    }

    defaultConfig {
        applicationId = "dev.hoppou.ret"
        minSdk = 21
        targetSdk = 29
        // set by `cog bump`, along with the version of the Rust crates
        versionName = "0.3.0"
        versionCode = versionCodeOf(versionName!!)
    }
    buildFeatures {
        buildConfig = true
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    signingConfigs {
        if (releaseStoreFile.isPresent) {
            create("release") {
                storeFile = file(releaseStoreFile.get())
                storePassword = providers.gradleProperty("RELEASE_STORE_PASSWORD").get()
                keyAlias = providers.gradleProperty("RELEASE_KEY_ALIAS").get()
                keyPassword = providers.gradleProperty("RELEASE_KEY_PASSWORD").get()
            }
        }
    }
    buildTypes {
        release {
            // drop the unused parts of the Kotlin standard library and resources from the apk
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release")
        }
    }
    lint {
        // ret is not distributed through Google Play, and raising targetSdk
        // changes foreground service and runtime behavior.
        disable += "ExpiredTargetSdkVersion"
    }
    testOptions {
        // the code under test logs through android.util.Log, which does nothing in unit tests
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    implementation(fileTree(mapOf("dir" to "libs", "include" to listOf("*.jar"))))
    testImplementation("junit:junit:4.13.2")
}

/**
 * The versionCode of a version: 10000 * major + 100 * minor + patch (3.0.2 gives 30002)
 *
 * It increases with every release, so that Android accepts the upgrade. ret computes the same
 * value from its own version (`VERSION_CODE` in crates/ret-adb/src/client.rs) to decide whether
 * the app on the device must be installed again.
 */
fun versionCodeOf(version: String): Int {
    val (major, minor, patch) = version.substringBefore('-').split('.').map(String::toInt)
    require(minor < 100 && patch < 100) { "Minor and patch must be below 100: $version" }
    return 10000 * major + 100 * minor + patch
}
