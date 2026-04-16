package com.inventyv.scannersigantureapp

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.util.Log
import android.widget.Button
import android.widget.ProgressBar
import android.widget.TextView
import android.view.SurfaceView
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

/**
 * Activity for document scanning
 */
class ScannerActivity : AppCompatActivity() {
    
    companion object {
        private const val TAG = "ScannerActivity"
        private const val CAMERA_PERMISSION_CODE = 100
    }
    
    private lateinit var surfaceView: SurfaceView
    private lateinit var cameraManager: CameraManager
    private lateinit var progressBar: ProgressBar
    private lateinit var statusText: TextView
    private lateinit var continueButton: Button
    private lateinit var cancelButton: Button
    
    private var scanResult = ""
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_scanner)
        
        // Initialize views
        surfaceView = findViewById(R.id.surfaceView)
        progressBar = findViewById(R.id.progressBar)
        statusText = findViewById(R.id.statusText)
        continueButton = findViewById(R.id.continueButton)
        cancelButton = findViewById(R.id.cancelButton)
        
        // Check camera permission
        if (ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.CAMERA
            ) == PackageManager.PERMISSION_GRANTED
        ) {
            startScanner()
        } else {
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.CAMERA),
                CAMERA_PERMISSION_CODE
            )
        }
        
        // Button listeners
        continueButton.setOnClickListener {
            if (scanResult.isNotEmpty()) {
                NativeBridge.processScanResult(scanResult)
                NativeBridge.nextScreen()
                finish()
            }
        }
        
        cancelButton.setOnClickListener {
            NativeBridge.previousScreen()
            finish()
        }
    }
    
    private fun startScanner() {
        cameraManager = CameraManager(
            this,
            surfaceView
        ) { result ->
            scanResult = result
            runOnUiThread {
                progressBar.visibility = android.view.View.GONE
                statusText.text = "✓ Scanned: $result"
                continueButton.isEnabled = true
            }
        }
    }
    
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        
        if (requestCode == CAMERA_PERMISSION_CODE) {
            if (grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
                startScanner()
            } else {
                Log.w(TAG, "Camera permission not granted")
                statusText.text = "Camera permission required"
                cancelButton.isEnabled = true
            }
        }
    }
    
    override fun onPause() {
        super.onPause()
        cameraManager.stopCamera()
    }
    
    override fun onDestroy() {
        cameraManager.stopCamera()
        super.onDestroy()
    }
}
