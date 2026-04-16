package com.inventyv.scannersigantureapp

import android.Manifest
import android.content.Context
import android.hardware.Camera
import android.util.Log
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import java.io.IOException

/**
 * Manages camera operations for document scanning
 */
class CameraManager(
    private val context: Context,
    private val surfaceView: SurfaceView,
    private val onScanResult: (String) -> Unit
) : SurfaceHolder.Callback {
    
    private var camera: Camera? = null
    private var isScanning = false
    private val scanDelay = 1000L // Delay between scans in ms
    private var lastScanTime = 0L
    
    companion object {
        const val TAG = "CameraManager"
    }
    
    init {
        surfaceView.holder.addCallback(this)
    }
    
    override fun surfaceCreated(holder: SurfaceHolder) {
        if (hasCameraPermission()) {
            startCamera(holder)
        }
    }
    
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        // Handle surface changes
    }
    
    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopCamera()
    }
    
    private fun startCamera(holder: SurfaceHolder) {
        try {
            camera = Camera.open(0)
            val params = camera?.parameters
            camera?.setPreviewDisplay(holder)
            camera?.startPreview()
            isScanning = true
            Log.d(TAG, "Camera started successfully")
        } catch (e: IOException) {
            Log.e(TAG, "Error starting camera", e)
        }
    }
    
    fun stopCamera() {
        try {
            camera?.stopPreview()
            camera?.release()
            camera = null
            isScanning = false
            Log.d(TAG, "Camera stopped")
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping camera", e)
        }
    }
    
    fun triggerScan() {
        val currentTime = System.currentTimeMillis()
        if (currentTime - lastScanTime >= scanDelay && isScanning) {
            lastScanTime = currentTime
            // Simulate barcode scan result
            val mockScanResult = "EAN-${System.currentTimeMillis()}"
            onScanResult(mockScanResult)
            Log.d(TAG, "Scan triggered: $mockScanResult")
        }
    }
    
    private fun hasCameraPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.CAMERA
        ) == android.content.pm.PackageManager.PERMISSION_GRANTED
    }
}
