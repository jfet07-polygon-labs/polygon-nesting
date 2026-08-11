#!/usr/bin/env python3
import json
import pathlib
import subprocess
import sys

WORKSPACE_CRATES = {
    "polygon-nesting-protocol",
    "polygon-nesting-core",
    "polygon-nesting-dxf",
    "polygon-nesting-cli",
    "polygon-nesting-napi",
}

ALLOWED_WORKSPACE_DEPENDENCIES = {
    "polygon-nesting-protocol": set(),
    "polygon-nesting-core": {"polygon-nesting-protocol"},
    "polygon-nesting-dxf": {"polygon-nesting-protocol"},
    "polygon-nesting-cli": {
        "polygon-nesting-protocol",
        "polygon-nesting-core",
        "polygon-nesting-dxf",
    },
    "polygon-nesting-napi": {
        "polygon-nesting-protocol",
        "polygon-nesting-core",
    },
}


def find_violations(metadata, workspace_root):
    workspace_root = pathlib.Path(workspace_root).resolve()
    member_ids = metadata["workspace_members"]
    packages_by_id = {}
    for package in metadata["packages"]:
        packages_by_id.setdefault(package["id"], []).append(package)

    violations = []
    if len(member_ids) != len(WORKSPACE_CRATES):
        violations.append(
            "workspace must contain exactly "
            f"{len(WORKSPACE_CRATES)} member IDs, found {len(member_ids)}"
        )
    if len(set(member_ids)) != len(member_ids):
        violations.append("workspace member IDs must be unique")

    workspace_packages = []
    for member_id in dict.fromkeys(member_ids):
        matching_packages = packages_by_id.get(member_id, [])
        if not matching_packages:
            violations.append(f"unresolved workspace member ID: {member_id}")
        elif len(matching_packages) > 1:
            violations.append(
                "workspace member ID resolves to "
                f"{len(matching_packages)} packages: {member_id}"
            )
        else:
            workspace_packages.append(matching_packages[0])

    packages_by_name = {}
    for package in workspace_packages:
        packages_by_name.setdefault(package["name"], []).append(package)

    workspace_crates = set(packages_by_name)
    missing_crates = sorted(WORKSPACE_CRATES - workspace_crates)
    for crate in missing_crates:
        violations.append(f"missing workspace crate: {crate}")

    unexpected_crates = sorted(workspace_crates - WORKSPACE_CRATES)
    for crate in unexpected_crates:
        violations.append(f"unexpected workspace crate: {crate}")

    for crate in sorted(WORKSPACE_CRATES & workspace_crates):
        matching_packages = packages_by_name[crate]
        if len(matching_packages) != 1:
            violations.append(
                "workspace package name resolves to "
                f"{len(matching_packages)} packages: {crate}"
            )
            continue

        package = matching_packages[0]
        manifest_path = pathlib.Path(package["manifest_path"])
        resolved_manifest_path = manifest_path.resolve()
        expected_manifest_path = (
            workspace_root / "crates" / crate / "Cargo.toml"
        ).resolve()
        if (
            not manifest_path.is_absolute()
            or manifest_path != resolved_manifest_path
            or resolved_manifest_path != expected_manifest_path
        ):
            violations.append(
                f"{crate} manifest path must be {expected_manifest_path}, "
                f"found {manifest_path}"
            )

        workspace_dependencies = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in workspace_crates
        }
        disallowed = sorted(
            workspace_dependencies - ALLOWED_WORKSPACE_DEPENDENCIES[crate]
        )
        for dependency in disallowed:
            violations.append(f"{crate} must not depend on {dependency}")

    return violations


def load_metadata(workspace_root):
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=workspace_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def main():
    if len(sys.argv) != 2:
        print(
            f"usage: {pathlib.Path(sys.argv[0]).name} WORKSPACE_ROOT",
            file=sys.stderr,
        )
        return 2

    workspace_root = pathlib.Path(sys.argv[1])
    violations = find_violations(load_metadata(workspace_root), workspace_root)
    if violations:
        for violation in violations:
            print(f"dependency direction violation: {violation}", file=sys.stderr)
        return 1

    print("dependency direction verified")
    return 0


if __name__ == "__main__":
    sys.exit(main())
