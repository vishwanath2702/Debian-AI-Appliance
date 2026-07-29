//! Root filesystem bootstrap abstraction.

use crate::BuildContext;

/// Creates a root filesystem from a build context.
pub trait Bootstrapper {
    /// Bootstrap-specific error.
    type Error;

    /// Creates the root filesystem described by the build context.
    ///
    /// # Errors
    ///
    /// Returns an error if the root filesystem cannot be created.
    fn bootstrap(&self, context: &BuildContext) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::Bootstrapper;
    use crate::{BootstrapConfig, BuildContext};

    struct RecordingBootstrapper;

    impl Bootstrapper for RecordingBootstrapper {
        type Error = Infallible;

        fn bootstrap(&self, context: &BuildContext) -> Result<(), Self::Error> {
            assert_eq!(context.bootstrap().release(), "bookworm");
            assert_eq!(context.rootfs(), std::path::Path::new("build/rootfs"));

            Ok(())
        }
    }

    #[test]
    fn bootstraps_from_build_context() {
        let bootstrap = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        let context = BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
            bootstrap,
        );

        RecordingBootstrapper.bootstrap(&context).unwrap();
    }
}
