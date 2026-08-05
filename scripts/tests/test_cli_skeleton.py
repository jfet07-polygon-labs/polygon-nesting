import pathlib
import tomllib
import unittest

CLI_CRATE = pathlib.Path(__file__).parents[2] / "crates" / "polygon-nesting-cli"
CLI_MANIFEST = CLI_CRATE / "Cargo.toml"
CLI_SOURCE = CLI_CRATE / "src" / "main.rs"


class CliSkeletonTests(unittest.TestCase):
    def test_keeps_ctrlc_dependency_for_planned_adapter(self):
        manifest = tomllib.loads(CLI_MANIFEST.read_text())

        self.assertIn("ctrlc", manifest["dependencies"])

    def test_installs_cooperative_sigint_handler(self):
        source = CLI_SOURCE.read_text()

        self.assertIn("ctrlc::set_handler", source)
        self.assertIn("CancelReason::Cancelled", source)


if __name__ == "__main__":
    unittest.main()
