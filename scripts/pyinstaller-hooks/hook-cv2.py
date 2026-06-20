import sys

from PyInstaller.utils.hooks import collect_data_files, collect_submodules

hiddenimports = ["numpy", "cv2.cv2"]
hiddenimports += collect_submodules("cv2", filter=lambda name: name != "cv2.load_config_py2")
excludedimports = ["cv2.load_config_py2"]

datas = collect_data_files(
    "cv2",
    include_py_files=True,
    includes=[
        "config.py",
        f"config-{sys.version_info[0]}.{sys.version_info[1]}.py",
        "config-3.py",
        "load_config_py3.py",
    ],
)

# The default PyInstaller cv2 hook also bundles OpenCV's ffmpeg video DLL on
# Windows. The OCR sidecar only reads static images, so skipping that DLL keeps
# the sidecar smaller without removing image decoding support.
binaries = []
module_collection_mode = "py"
