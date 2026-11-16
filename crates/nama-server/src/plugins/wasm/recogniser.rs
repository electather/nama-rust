//! An Installed Plugin as a Recogniser: the Kernel's `Recogniser` calls passed through as the
//! contract's `RecogniserService` rpcs.

use buffa::Message;
use nama_plugin_api::{
    BoxFuture, PluginError,
    contract::{FileRecognised, FileToRecognise, TitleRecognised, TitleToRecognise},
    recogniser::{Recogniser, RecogniserMethod},
};

use super::{
    Running,
    services::{Method, RECOGNISE_FILE, RECOGNISE_TITLE},
};

impl Running {
    /// Calls `method` if the Plugin serves it. One it doesn't serve answers that it can't tell,
    /// without a call.
    fn recognise<'a, Req: Message, Resp: Message + Default + 'a>(
        &'a self,
        method: &'static Method,
        request: &'a Req,
    ) -> BoxFuture<'a, Result<Resp, PluginError>> {
        Box::pin(async move {
            if !self.serves(method) {
                return Ok(Resp::default());
            }
            self.call_method(method, request).await
        })
    }
}

impl Recogniser for Running {
    fn recognise_title<'a>(
        &'a self,
        title: &'a TitleToRecognise,
    ) -> BoxFuture<'a, Result<TitleRecognised, PluginError>> {
        self.recognise(&RECOGNISE_TITLE, title)
    }

    fn recognise_file<'a>(
        &'a self,
        file: &'a FileToRecognise,
    ) -> BoxFuture<'a, Result<FileRecognised, PluginError>> {
        self.recognise(&RECOGNISE_FILE, file)
    }

    fn recognises(&self, method: RecogniserMethod) -> bool {
        self.serves(match method {
            RecogniserMethod::Title => &RECOGNISE_TITLE,
            RecogniserMethod::File => &RECOGNISE_FILE,
        })
    }
}

#[cfg(test)]
#[path = "recogniser_tests.rs"]
mod tests;
