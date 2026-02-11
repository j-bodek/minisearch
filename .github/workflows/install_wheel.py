from packaging.tags import sys_tags
import glob, os, sys, subprocess

dist = sys.argv[1] if len(sys.argv) > 1 else "dist"
wheels = glob.glob(os.path.join(dist, "*.whl"))
if not wheels:
    raise SystemExit(f"No wheels found in {dist}/")

compatible_tags = [str(tag) for tag in sys_tags()]
matches = [w for w in wheels if any([tag in w for tag in compatible_tags])]

if not matches:
    print("Compatible tags: ", *[f"- {t}" for t in compatible_tags], sep="\n ")
    print(
        "Available wheels:", *[f"- {os.path.basename(w)}" for w in wheels], sep="\n  "
    )
    raise SystemExit(f"No compatible wheel found")

wheel = sorted(matches)[0]
print("Installing:", os.path.basename(wheel))
subprocess.check_call(
    [sys.executable, "-m", "pip", "install", "--force-reinstall", wheel]
)
