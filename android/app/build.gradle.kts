plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

import java.io.File

android {
    namespace = "com.inventyv.xilemgameactivity"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.inventyv.xilemgameactivity"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters.add("arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
}

val rustTarget = "aarch64-linux-android"
val rustLibName = "libscanner_signature_app.so"
val rustProjectDir = rootProject.projectDir.parentFile
val rustOutputDir = File(rustProjectDir, "target/$rustTarget/release")
val androidJniDir = File(projectDir, "src/main/jniLibs/arm64-v8a")

val buildRustLib by tasks.registering(Exec::class) {
    workingDir = rustProjectDir
    commandLine("cargo", "build", "--release", "--target", rustTarget)
}

val copyRustLib by tasks.registering(Copy::class) {
    dependsOn(buildRustLib)
    from(File(rustOutputDir, rustLibName))
    into(androidJniDir)
}

tasks.named("preBuild") {
    dependsOn(copyRustLib)
}

dependencies {

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation("androidx.games:games-activity:4.4.0")
    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
}