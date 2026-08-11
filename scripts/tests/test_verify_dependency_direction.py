import importlib.util
import pathlib
import unittest

SCRIPT_PATH = pathlib.Path(__file__).parents[1] / "verify_dependency_direction.py"
SPEC = importlib.util.spec_from_file_location("verify_dependency_direction", SCRIPT_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class VerifyDependencyDirectionTests(unittest.TestCase):
    workspace_root = pathlib.Path("/workspace")

    def package(self, name, dependencies):
        return {
            "id": f"path+file:///workspace/{name}#0.1.0",
            "name": name,
            "manifest_path": str(
                self.workspace_root / "crates" / name / "Cargo.toml"
            ),
            "dependencies": [{"name": dependency} for dependency in dependencies],
        }

    def metadata(self, dependencies, roles=None, extra_packages=None):
        package_names = {
            "protocol": "polygon-nesting-protocol",
            "core": "polygon-nesting-core",
            "dxf": "polygon-nesting-dxf",
            "cli": "polygon-nesting-cli",
            "napi": "polygon-nesting-napi",
        }
        selected_roles = package_names if roles is None else roles
        packages = [
            self.package(package_names[role], dependencies.get(role, []))
            for role in selected_roles
        ]
        packages.extend(
            self.package(name, package_dependencies)
            for name, package_dependencies in (extra_packages or {}).items()
        )
        return {"packages": packages, "workspace_members": [p["id"] for p in packages]}

    def test_accepts_expected_dependency_direction(self):
        metadata = self.metadata(
            {
                "protocol": [],
                "core": ["polygon-nesting-protocol"],
                "dxf": ["polygon-nesting-protocol"],
                "cli": [
                    "polygon-nesting-core",
                    "polygon-nesting-dxf",
                    "polygon-nesting-protocol",
                ],
                "napi": ["polygon-nesting-core", "polygon-nesting-protocol"],
            }
        )

        self.assertEqual(MODULE.find_violations(metadata, self.workspace_root), [])

    def test_rejects_missing_required_workspace_crate(self):
        metadata = self.metadata(
            {
                "protocol": [],
                "core": ["polygon-nesting-protocol"],
                "dxf": ["polygon-nesting-protocol"],
                "cli": ["polygon-nesting-core", "polygon-nesting-protocol"],
            },
            roles=("protocol", "core", "dxf", "cli"),
        )

        self.assertEqual(
            MODULE.find_violations(metadata, self.workspace_root),
            [
                "workspace must contain exactly 5 member IDs, found 4",
                "missing workspace crate: polygon-nesting-napi",
            ],
        )

    def test_rejects_extra_workspace_crate(self):
        metadata = self.metadata(
            {
                "protocol": [],
                "core": ["polygon-nesting-protocol"],
                "dxf": ["polygon-nesting-protocol"],
                "cli": ["polygon-nesting-core", "polygon-nesting-protocol"],
                "napi": ["polygon-nesting-core", "polygon-nesting-protocol"],
            },
            extra_packages={"polygon-nesting-helper": []},
        )

        self.assertEqual(
            MODULE.find_violations(metadata, self.workspace_root),
            [
                "workspace must contain exactly 5 member IDs, found 6",
                "unexpected workspace crate: polygon-nesting-helper",
            ],
        )

    def test_rejects_wrong_workspace_member_id_count(self):
        metadata = self.metadata({})
        metadata["workspace_members"].append(
            "path+file:///workspace/polygon-nesting-helper#0.1.0"
        )

        self.assertIn(
            "workspace must contain exactly 5 member IDs, found 6",
            MODULE.find_violations(metadata, self.workspace_root),
        )

    def test_rejects_duplicate_workspace_member_ids(self):
        metadata = self.metadata({})
        metadata["workspace_members"][1] = metadata["workspace_members"][0]

        self.assertIn(
            "workspace member IDs must be unique",
            MODULE.find_violations(metadata, self.workspace_root),
        )

    def test_rejects_dangling_workspace_member_id(self):
        metadata = self.metadata({})
        dangling_id = "path+file:///workspace/missing#0.1.0"
        metadata["workspace_members"][0] = dangling_id

        self.assertIn(
            f"unresolved workspace member ID: {dangling_id}",
            MODULE.find_violations(metadata, self.workspace_root),
        )

    def test_rejects_ambiguous_workspace_member_resolution(self):
        metadata = self.metadata({})
        member_id = metadata["workspace_members"][0]
        metadata["packages"].append(dict(metadata["packages"][0]))

        self.assertIn(
            f"workspace member ID resolves to 2 packages: {member_id}",
            MODULE.find_violations(metadata, self.workspace_root),
        )

    def test_rejects_noncanonical_manifest_path(self):
        metadata = self.metadata({})
        package = metadata["packages"][0]
        package["manifest_path"] = "/workspace/other/Cargo.toml"
        expected_path = self.workspace_root / "crates" / package["name"] / "Cargo.toml"

        expected_violation = (
            f"{package['name']} manifest path must be {expected_path}, "
            "found /workspace/other/Cargo.toml"
        )
        self.assertIn(
            expected_violation,
            MODULE.find_violations(metadata, self.workspace_root),
        )

    def test_rejects_role_manifest_path_swap(self):
        metadata = self.metadata({})
        protocol = metadata["packages"][0]
        core = metadata["packages"][1]
        protocol["manifest_path"], core["manifest_path"] = (
            core["manifest_path"],
            protocol["manifest_path"],
        )

        violations = MODULE.find_violations(metadata, self.workspace_root)
        self.assertIn(
            "polygon-nesting-protocol manifest path must be "
            "/workspace/crates/polygon-nesting-protocol/Cargo.toml, found "
            "/workspace/crates/polygon-nesting-core/Cargo.toml",
            violations,
        )
        self.assertIn(
            "polygon-nesting-core manifest path must be "
            "/workspace/crates/polygon-nesting-core/Cargo.toml, found "
            "/workspace/crates/polygon-nesting-protocol/Cargo.toml",
            violations,
        )

    def test_rejects_each_inverted_workspace_dependency(self):
        cases = {
            "protocol_to_core": {"protocol": ["polygon-nesting-core"]},
            "protocol_to_cli": {"protocol": ["polygon-nesting-cli"]},
            "protocol_to_napi": {"protocol": ["polygon-nesting-napi"]},
            "protocol_to_dxf": {"protocol": ["polygon-nesting-dxf"]},
            "core_to_cli": {"core": ["polygon-nesting-cli"]},
            "core_to_napi": {"core": ["polygon-nesting-napi"]},
            "core_to_dxf": {"core": ["polygon-nesting-dxf"]},
            "dxf_to_core": {"dxf": ["polygon-nesting-core"]},
            "dxf_to_cli": {"dxf": ["polygon-nesting-cli"]},
            "dxf_to_napi": {"dxf": ["polygon-nesting-napi"]},
            "cli_to_napi": {"cli": ["polygon-nesting-napi"]},
            "napi_to_cli": {"napi": ["polygon-nesting-cli"]},
        }

        for name, dependencies in cases.items():
            with self.subTest(name=name):
                self.assertTrue(
                    MODULE.find_violations(
                        self.metadata(dependencies), self.workspace_root
                    )
                )

    def test_rejects_unexpected_workspace_crate_edge(self):
        metadata = self.metadata(
            {"protocol": ["polygon-nesting-helper"]},
            extra_packages={"polygon-nesting-helper": []},
        )

        self.assertEqual(
            MODULE.find_violations(metadata, self.workspace_root),
            [
                "workspace must contain exactly 5 member IDs, found 6",
                "unexpected workspace crate: polygon-nesting-helper",
                "polygon-nesting-protocol must not depend on polygon-nesting-helper",
            ],
        )

    def test_ignores_external_dependencies(self):
        metadata = self.metadata(
            {
                "protocol": ["serde"],
                "core": ["rand"],
                "dxf": ["sha2"],
                "cli": ["clap"],
                "napi": ["napi"],
            }
        )

        self.assertEqual(MODULE.find_violations(metadata, self.workspace_root), [])


if __name__ == "__main__":
    unittest.main()
