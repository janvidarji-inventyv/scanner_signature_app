package com.inventyv.scannersigantureapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity

class SuccessActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_success)
        
        Log.d("SuccessActivity", "Success screen initialized")
    }
}
