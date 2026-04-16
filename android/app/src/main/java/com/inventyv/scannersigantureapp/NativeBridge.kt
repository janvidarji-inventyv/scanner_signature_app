package com.inventyv.scannersigantureapp

import android.util.Log
import org.json.JSONObject

object NativeBridge {
    init {
        System.loadLibrary("scanner_signature_app")
    }

    // ============ App Lifecycle ============
    external fun initializeApp(): String
    external fun getAppState(): String
    external fun resetApp(): String

    // ============ Navigation ============
    external fun nextScreen(): String
    external fun previousScreen(): String

    // ============ Scanner ============
    external fun processScanResult(scanResult: String)

    // ============ Signature Handling ============
    external fun addSignaturePoint(x: Float, y: Float, pressure: Float)
    external fun clearSignature()
    external fun saveSignature(): String
    external fun getSignaturePointsCount(): Int
    external fun isSignatureValid(): Boolean

    // Helper functions
    fun initializeBridge() {
        try {
            val initStatus = initializeApp()
            Log.d("NativeBridge", "Initialized: $initStatus")
        } catch (e: Exception) {
            Log.e("NativeBridge", "Error initializing bridge", e)
        }
    }

    fun getCurrentState(): Map<String, Any>? {
        return try {
            val stateJson = getAppState()
            val parser = JSONObject(stateJson)
            mapOf(
                "screen" to parser.optString("current_screen", "Unknown"),
                "hasScans" to (parser.optJSONObject("scan_data") != null),
                "hasSignature" to (parser.optJSONObject("signature_data") != null)
            )
        } catch (e: Exception) {
            Log.e("NativeBridge", "Error parsing state", e)
            null
        }
    }
}

