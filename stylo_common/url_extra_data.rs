/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Extra data that the backend may need to resolve url values.

#![allow(unsafe_code)]

#[cfg(feature = "gecko")]
use crate::gecko_bindings::{bindings, structs, sugar::refptr::RefCounted};
#[cfg(feature = "servo")]
use servo_arc::Arc;
#[cfg(feature = "gecko")]
use std::fmt;
#[cfg(feature = "gecko")]
use std::mem;
#[cfg(feature = "gecko")]
use std::mem::ManuallyDrop;
use to_shmem::{SharedMemoryBuilder, ToShmem};

/// Extra data that the backend may need to resolve url values.
///
/// If the usize's lowest bit is 0, then this is a strong reference to a
/// structs::URLExtraData object.
///
/// Otherwise, shifting the usize's bits the right by one gives the
/// UserAgentStyleSheetID value corresponding to the style sheet whose
/// URLExtraData this is, which is stored in URLExtraData_sShared.  We don't
/// hold a strong reference to that object from here, but we rely on that
/// array's objects being held alive until shutdown.
///
/// We use this packed representation rather than an enum so that
/// `from_ptr_ref` can work.
#[cfg(feature = "gecko")]
// Although deriving MallocSizeOf means it always returns 0, that is fine because UrlExtraData
// objects are reference-counted.
#[derive(MallocSizeOf, PartialEq)]
#[repr(C)]
pub struct UrlExtraData(usize);

/// Extra data that the backend may need to resolve url values.
#[cfg(feature = "servo")]
#[derive(Clone, Debug, Eq, MallocSizeOf, PartialEq)]
pub struct UrlExtraData(#[ignore_malloc_size_of = "Arc"] pub Arc<::url::Url>);

#[cfg(feature = "servo")]
impl UrlExtraData {
    /// True if this URL scheme is chrome.
    pub fn chrome_rules_enabled(&self) -> bool {
        self.0.scheme() == "chrome"
    }

    /// Get the interior Url as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(feature = "servo")]
impl From<::url::Url> for UrlExtraData {
    fn from(url: ::url::Url) -> Self {
        Self(Arc::new(url))
    }
}

#[cfg(not(feature = "gecko"))]
impl ToShmem for UrlExtraData {
    fn to_shmem(&self, _builder: &mut SharedMemoryBuilder) -> to_shmem::Result<Self> {
        unimplemented!("If servo wants to share stylesheets across processes, ToShmem for Url must be implemented");
    }
}

#[cfg(feature = "gecko")]
impl Clone for UrlExtraData {
    fn clone(&self) -> UrlExtraData {
        UrlExtraData::new(self.ptr())
    }
}

#[cfg(feature = "gecko")]
impl Drop for UrlExtraData {
    fn drop(&mut self) {
        // No need to release when we have an index into URLExtraData_sShared.
        if self.0 & 1 == 0 {
            unsafe {
                self.as_ref().release();
            }
        }
    }
}

#[cfg(feature = "gecko")]
impl ToShmem for UrlExtraData {
    fn to_shmem(&self, _builder: &mut SharedMemoryBuilder) -> to_shmem::Result<Self> {
        if self.0 & 1 == 0 {
            let shared_extra_datas = unsafe {
                std::ptr::addr_of!(structs::URLExtraData_sShared)
                    .as_ref()
                    .unwrap()
            };
            let self_ptr = self.as_ref() as *const _ as *mut _;
            let sheet_id = shared_extra_datas
                .iter()
                .position(|r| r.mRawPtr == self_ptr);
            let sheet_id = match sheet_id {
                Some(id) => id,
                None => {
                    return Err(String::from(
                        "ToShmem failed for UrlExtraData: expected sheet's URLExtraData to be in \
                         URLExtraData::sShared",
                    ));
                },
            };
            Ok(ManuallyDrop::new(UrlExtraData((sheet_id << 1) | 1)))
        } else {
            Ok(ManuallyDrop::new(UrlExtraData(self.0)))
        }
    }
}

#[cfg(feature = "gecko")]
impl UrlExtraData {
    /// Create a new UrlExtraData wrapping a pointer to the specified Gecko
    /// URLExtraData object.
    pub fn new(ptr: *mut structs::URLExtraData) -> UrlExtraData {
        unsafe {
            (*ptr).addref();
        }
        UrlExtraData(ptr as usize)
    }

    /// True if this URL scheme is chrome.
    #[inline]
    pub fn chrome_rules_enabled(&self) -> bool {
        self.as_ref().mChromeRulesEnabled
    }

    /// Create a reference to this `UrlExtraData` from a reference to pointer.
    ///
    /// The pointer must be valid and non null.
    ///
    /// This method doesn't touch refcount.
    #[inline]
    pub unsafe fn from_ptr_ref(ptr: &*mut structs::URLExtraData) -> &Self {
        mem::transmute(ptr)
    }

    /// Returns a pointer to the Gecko URLExtraData object.
    pub fn ptr(&self) -> *mut structs::URLExtraData {
        if self.0 & 1 == 0 {
            self.0 as *mut structs::URLExtraData
        } else {
            unsafe {
                let sheet_id = self.0 >> 1;
                structs::URLExtraData_sShared[sheet_id].mRawPtr
            }
        }
    }

    fn as_ref(&self) -> &structs::URLExtraData {
        unsafe { &*(self.ptr() as *const structs::URLExtraData) }
    }
}

#[cfg(feature = "gecko")]
impl fmt::Debug for UrlExtraData {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        macro_rules! define_debug_struct {
            ($struct_name:ident, $gecko_class:ident, $debug_fn:ident) => {
                struct $struct_name(*mut structs::$gecko_class);
                impl fmt::Debug for $struct_name {
                    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        use nsstring::nsCString;
                        let mut spec = nsCString::new();
                        unsafe {
                            bindings::$debug_fn(self.0, &mut spec);
                        }
                        spec.fmt(formatter)
                    }
                }
            };
        }

        define_debug_struct!(DebugURI, nsIURI, Gecko_nsIURI_Debug);
        define_debug_struct!(
            DebugReferrerInfo,
            nsIReferrerInfo,
            Gecko_nsIReferrerInfo_Debug
        );

        formatter
            .debug_struct("URLExtraData")
            .field("chrome_rules_enabled", &self.chrome_rules_enabled())
            .field("base", &DebugURI(self.as_ref().mBaseURI.raw()))
            .field(
                "referrer",
                &DebugReferrerInfo(self.as_ref().mReferrerInfo.raw()),
            )
            .finish()
    }
}

// XXX We probably need to figure out whether we should mark Eq here.
// It is currently marked so because properties::UnparsedValue wants Eq.
#[cfg(feature = "gecko")]
impl Eq for UrlExtraData {}
