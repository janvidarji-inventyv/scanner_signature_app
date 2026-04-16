package com.inventyv.scannersigantureapp

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import com.google.mlkit.vision.barcode.scanning.BarcodeScanner
import com.google.mlkit.vision.common.InputImage
import android.provider.MediaStore
import android.graphics.Bitmap

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize Xilem app
        val initMessage = NativeBridge.initApp()
        println("Init: $initMessage")
    }

    fun openScanner() {
        val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE)
        startActivityForResult(intent, CAMERA_REQUEST_CODE)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        
        if (requestCode == CAMERA_REQUEST_CODE && resultCode == RESULT_OK) {
            val bitmap = data?.extras?.get("data") as? Bitmap
            bitmap?.let {
                val scanResult = "Barcode: ${System.currentTimeMillis()}"
                NativeBridge.processScanResult(scanResult)
            }
        }
    }

    companion object {
        private const val CAMERA_REQUEST_CODE = 100
    }
}
