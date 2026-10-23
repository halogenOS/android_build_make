#!/usr/bin/env python3
#
# SPDX-FileCopyrightText: The halogenOS Project
# SPDX-License-Identifier: Apache-2.0

"""Sets the release-keys tag in a target-files directory.

Edits the build tags in every build property file and in the AVB properties
of META/misc_info.txt as sign_target_files_apks does, so the images built
from the directory report release-keys.
"""

import argparse
import os

import common
import sign_target_files_apks


def set_release_tags(target_files_dir):
  """Edits the build tags in target_files_dir in place."""
  for root, _, files in os.walk(target_files_dir):
    for name in files:
      path = os.path.join(root, name)
      # A link can point outside the directory.
      if os.path.islink(path) or not sign_target_files_apks.IsBuildPropFile(
          os.path.relpath(path, target_files_dir)):
        continue
      with open(path) as prop_file:
        props = prop_file.read()
      with open(path, "w") as prop_file:
        prop_file.write(sign_target_files_apks.RewriteProps(props))

  misc_info_path = os.path.join(target_files_dir, "META", "misc_info.txt")
  misc_info = common.LoadDictionaryFromFile(misc_info_path)
  if misc_info.get("avb_enable") == "true":
    sign_target_files_apks.RewriteAvbProps(misc_info)
    with open(misc_info_path, "w") as misc_info_file:
      misc_info_file.writelines(
          f"{key}={value}\n" for key, value in sorted(misc_info.items()))


def main():
  parser = argparse.ArgumentParser(description=__doc__)
  parser.add_argument("target_files_dir",
                      help="target-files directory, edited in place")
  set_release_tags(parser.parse_args().target_files_dir)


if __name__ == "__main__":
  main()
