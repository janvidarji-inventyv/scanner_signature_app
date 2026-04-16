package com.inventyv.scannersigantureapp

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.app.ActivityCompat

class MainActivity : AppCompatActivity() {
    companion object {
        const val CAMERA_REQUEST_CODE = 100
        const val PERMISSION_REQUEST_CODE = 200
        private const val TAG = "MainActivity"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        // Initialize Rust Xilem app
        try {
            NativeBridge.initializeBridge()
            Log.d(TAG, "Rust app initialized successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Error initializing Rust app", e)
        }

        // Request camera permissions if needed
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            if (ContextCompat.checkSelfPermission(
                    this,
                    Manifest.permission.CAMERA
                ) != android.content.pm.PackageManager.PERMISSION_GRANTED
            ) {
                ActivityCompat.requestPermissions(
                    this,
                    arrayOf(Manifest.permission.CAMERA),
                    PERMISSION_REQUEST_CODE
                )
            }
        }
    }

    /**
     * Start camera for scanning - called from Rust UI
     */
    fun openScanner() {
        if (hasCameraPermission()) {
            val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE)
            startActivityForResult(intent, CAMERA_REQUEST_CODE)
        } else {
            requestCameraPermission()
        }
    }

    /**
     * Process camera result
     */
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)

        if (requestCode == CAMERA_REQUEST_CODE && resultCode == RESULT_OK) {
            // Simulate scan result
            val scanResult = generateMockScanResult()
            NativeBridge.processScanResult(scanResult)
            Log.d(TAG, "Scan result processed: $scanResult")
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)

        if (requestCode == PERMISSION_REQUEST_CODE) {
            if (grantResults.isNotEmpty() &&
                grantResults[0] == android.content.pm.PackageManager.PERMISSION_GRANTED
            ) {
                openScanner()
            } else {
                Log.w(TAG, "Camera permission denied")
            }
        }
    }

    private fun hasCameraPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.CAMERA
        ) == android.content.pm.PackageManager.PERMISSION_GRANTED
    }

    private fun requestCameraPermission() {
        ActivityCompat.requestPermissions(
            this,
            arrayOf(Manifest.permission.CAMERA),
            PERMISSION_REQUEST_CODE
        )
    }

    private fun generateMockScanResult(): String {
        return "DOC-${System.currentTimeMillis()}"
    }
}
