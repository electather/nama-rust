//! Provide: a Recogniser, which proposes what a file is or which title an Item names. The Kernel
//! decides: it owns every Match and the Unmatched decision, and falls back on its own Matching
//! (ADR 0050).
//!
//! A Plugin serves it by registering an `Arc<dyn Recogniser>`:
//!
//! ```ignore
//! registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(my_recogniser));
//! ```

use crate::{
    BoxFuture, PluginError,
    contract::{FileRecognised, FileToRecognise, TitleRecognised, TitleToRecognise},
};

/// A Recogniser. It speaks the contract's `RecogniserService` messages, as an Installed Plugin's
/// exports do. An answer left empty means it can't tell, and the Kernel asks the next Recogniser.
///
/// Both methods are optional: one left out answers that it can't tell, so one point serves
/// Recognisers that answer only one of them.
pub trait Recogniser: Send + Sync + 'static {
    /// The ids of the title an Item names, as `title` describes it, which the title sources'
    /// search couldn't settle; empty if it can't tell.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which leaves the Item undecided until the next Scan.
    fn recognise_title<'a>(
        &'a self,
        _title: &'a TitleToRecognise,
    ) -> BoxFuture<'a, Result<TitleRecognised, PluginError>> {
        cant_tell()
    }

    /// What `file` is, a file the Kernel's name parser made no Item from; unset if it can't
    /// tell.
    ///
    /// # Errors
    ///
    /// When it couldn't answer, which leaves the file unparsed until the next Scan.
    fn recognise_file<'a>(
        &'a self,
        _file: &'a FileToRecognise,
    ) -> BoxFuture<'a, Result<FileRecognised, PluginError>> {
        cant_tell()
    }

    /// Whether this Recogniser serves `method`. The Kernel asks it only for the methods it
    /// answers; a Built-in that leaves a method out may keep the default, which costs one empty
    /// answer.
    fn recognises(&self, _method: RecogniserMethod) -> bool {
        true
    }
}

/// A [`Recogniser`] method the Kernel may ask, for [`Recogniser::recognises`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecogniserMethod {
    /// [`Recogniser::recognise_title`].
    Title,
    /// [`Recogniser::recognise_file`].
    File,
}

/// The answer of a [`Recogniser`] method its Recogniser left out.
fn cant_tell<'a, T: Default + Send + 'a>() -> BoxFuture<'a, Result<T, PluginError>> {
    Box::pin(std::future::ready(Ok(T::default())))
}
