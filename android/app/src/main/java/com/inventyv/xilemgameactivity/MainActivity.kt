package com.inventyv.scanner_signature_app

import com.google.androidgamesdk.GameActivity

class MainActivity : GameActivity() {
    companion object {
        init {
            System.loadLibrary("scanner_signature_app")
        }
    }
}
