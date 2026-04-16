use xilem::{view::*, Pod, View, ViewId};

pub fn app_launch_screen() -> impl View<()> {
    flex_column((
        flex_row((
            label("App Launcher"),
        )).with_flex_child(label("Scan & Sign"), 1.0),
        spacer(0.0, 20.0),
        label("v1.0.0"),
        spacer(0.0, 10.0),
        label("Ready to scan documents and capture signatures"),
        spacer(0.0, 30.0),
        button("Start Scan", ()).with_flex_child(label(""), 1.0),
    ))
}

pub fn scanner_info_screen(scan_result: &str) -> impl View<()> {
    flex_column((
        label("Scanner Result"),
        spacer(0.0, 15.0),
        label(&format!("Scanned: {}", scan_result)),
        spacer(0.0, 20.0),
        button("Continue to Signature", ()),
    ))
}

pub fn signature_info_screen() -> impl View<()> {
    flex_column((
        label("Signature Required"),
        spacer(0.0, 10.0),
        label("Please sign below to confirm"),
        spacer(0.0, 20.0),
        button("Draw Signature", ()),
    ))
}

pub fn signature_pad_screen() -> impl View<()> {
    flex_column((
        label("Draw your signature"),
        spacer(0.0, 200.0), // Canvas space placeholder
        flex_row((
            button("Cancel", ()),
            button("Clear", ()),
            button("Accept", ()),
        )),
    ))
}

pub fn signature_preview_screen() -> impl View<()> {
    flex_column((
        label("Preview Signature"),
        spacer(0.0, 150.0), // Preview area
        flex_row((
            button("Edit", ()),
            button("Save", ()),
        )),
    ))
}

pub fn success_screen() -> impl View<()> {
    flex_column((
        label("✓ Success"),
        spacer(0.0, 20.0),
        label("Document signed and saved successfully"),
        spacer(0.0, 30.0),
        button("Home", ()),
    ))
}