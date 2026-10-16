"""Run chapter pagination tests with only the HTTP boundary replaced.

Usage: python3 tests/chapter_order/run.py
Requires Python 3 and Cargo. The production method is compiled verbatim in a
small crate so these tests do not need the desktop GUI dependencies.
"""
import pathlib
import shutil
import subprocess
import tempfile
import json

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent

with tempfile.TemporaryDirectory(prefix="chapter-order-") as temporary:
    target = pathlib.Path(temporary)
    (target / "src").mkdir()
    source = (ROOT / "src-tauri/src/copy_client.rs").read_text()
    start = source.index("    pub async fn get_group_chapters(")
    end = source.index("\n    #[instrument(", start)
    template = (HERE / "harness.rs").read_text()
    assert template.count("/* PRODUCTION_METHOD */") == 1
    (target / "src/lib.rs").write_text(template.replace("/* PRODUCTION_METHOD */", source[start:end]))
    for name in ("Cargo.toml", "Cargo.lock"):
        shutil.copyfile(HERE / name, target / name)
    # Numeric order has gaps and titles run in reverse lexical order.
    chapters = [dict(uuid=f"uuid-{i}", name=f"Chapter {1000-i}", ordered=i*10-20)
                for i in range(447)]
    pages = [dict(offset=offset, total=len(chapters), list=chapters[offset:offset+100])
             for offset in range(0, len(chapters), 100)]
    (target / "pages.json").write_text(json.dumps(pages))
    subprocess.run(["cargo", "test", "--locked", "--manifest-path", str(target / "Cargo.toml"),
                    "--", "--nocapture", "--test-threads=1"], check=True)
