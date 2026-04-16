package com.inventyv.scannersigantureapp

object NativeBridge {
    init {
        System.loadLibrary("scanner_signature_app")
    }

   external fun initApp(): String
    external fun startScanner()
    external fun processScanResult(scanResult: String)
    external fun saveSignature(signatureJson: String)
}
