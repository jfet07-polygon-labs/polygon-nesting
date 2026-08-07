import pathlib
import tomllib
import unittest

CLI_CRATE = pathlib.Path(__file__).parents[2] / "crates" / "polygon-nesting-cli"
CLI_MANIFEST = CLI_CRATE / "Cargo.toml"
CLI_SOURCE = CLI_CRATE / "src" / "main.rs"


class CliSkeletonTests(unittest.TestCase):
    def test_uses_direct_unix_signal_handling_and_keeps_the_non_unix_adapter(self):
        manifest = tomllib.loads(CLI_MANIFEST.read_text())

        self.assertIn(
            "signal-hook-registry",
            manifest["target"]["cfg(unix)"]["dependencies"],
        )
        self.assertIn("ctrlc", manifest["target"]["cfg(not(unix))"]["dependencies"])

    def test_installs_cooperative_sigint_and_sigterm_handlers(self):
        source = CLI_SOURCE.read_text()

        self.assertIn("register(nix::libc::SIGINT", source)
        self.assertIn("register(nix::libc::SIGTERM", source)
        self.assertIn("ctrlc::set_handler", source)
        self.assertIn("CancelReason::Cancelled", source)


if __name__ == "__main__":
    unittest.main()
