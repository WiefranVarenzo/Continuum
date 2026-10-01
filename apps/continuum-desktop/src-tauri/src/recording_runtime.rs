//! Probe the actual loaded GStreamer runtime, not merely files or portal sockets.
//! Instantiating these elements in NULL state never opens a capture device.
use std::ffi::{c_char, c_void, CStr};

#[link(name = "libgstreamer-1.0.so.0", kind = "dylib", modifiers = "+verbatim")]
unsafe extern "C" {
    fn gst_init_check(argc: *mut i32, argv: *mut *mut *mut c_char, error: *mut *mut c_void) -> i32;
    fn gst_element_factory_make(factory: *const c_char, name: *const c_char) -> *mut c_void;
    fn gst_object_unref(object: *mut c_void);
}

pub fn missing_elements() -> Vec<String> {
    // Stable GStreamer C ABI. Null argv/error are supported; every successfully
    // created element owns one reference that is released without changing state.
    unsafe {
        if gst_init_check(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
            return vec!["GStreamer initialization".into()];
        }
    }
    let required: &[&CStr] = &[
        c"uritranscodebin", c"encodebin2", c"appsrc", c"appsink", c"queue",
        c"videoconvert", c"videorate", c"autovideoflip", c"vp8enc", c"vp8dec",
        c"audioconvert", c"audioresample", c"audiorate", c"opusenc", c"opusdec",
        c"opusparse", c"webmmux", c"matroskademux", c"playbin3", c"volume",
        // Actual PipeWire screens can negotiate DMA-BUF, unlike CPU-only
        // canvas probes. WebKit must be able to import and convert GL frames.
        c"glupload", c"glcolorconvert", c"gldownload", c"glsinkbin",
    ];
    required.iter().filter_map(|name| {
        let element = unsafe { gst_element_factory_make(name.as_ptr(), std::ptr::null()) };
        if element.is_null() { Some(name.to_string_lossy().into_owned()) }
        else { unsafe { gst_object_unref(element); } None }
    }).collect()
}

pub fn missing_native_screen_elements() -> Vec<String> {
    unsafe {
        if gst_init_check(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
            return vec!["GStreamer initialization".into()];
        }
    }
    [c"pipewiresrc", c"videoconvert", c"videorate", c"identity", c"vp8enc", c"webmmux", c"filesink"]
        .iter()
        .filter_map(|name| {
            let element = unsafe { gst_element_factory_make(name.as_ptr(), std::ptr::null()) };
            if element.is_null() { Some(name.to_string_lossy().into_owned()) }
            else { unsafe { gst_object_unref(element); } None }
        })
        .collect()
}

pub fn missing_native_screen_audio_elements() -> Vec<String> {
    unsafe {
        if gst_init_check(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
            return vec!["GStreamer initialization".into()];
        }
    }
    [c"pulsesrc", c"audiomixer", c"audioconvert", c"audioresample", c"rawaudioparse", c"opusenc", c"queue", c"volume"]
        .iter()
        .filter_map(|name| {
            let element = unsafe { gst_element_factory_make(name.as_ptr(), std::ptr::null()) };
            if element.is_null() { Some(name.to_string_lossy().into_owned()) }
            else { unsafe { gst_object_unref(element); } None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn installed_runtime_can_construct_recording_elements_without_sensors() {
        assert_eq!(super::missing_elements(), Vec::<String>::new());
    }
}
