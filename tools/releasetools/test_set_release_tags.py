#
# SPDX-FileCopyrightText: The halogenOS Project
# SPDX-License-Identifier: Apache-2.0

import os

import common
import test_utils
from set_release_tags import set_release_tags


class SetReleaseTagsTest(test_utils.ReleaseToolsTestCase):

  def setUp(self):
    self.target_files_dir = common.MakeTempDir()

  def write(self, name, content):
    path = os.path.join(self.target_files_dir, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
      f.write(content)

  def read(self, name):
    with open(os.path.join(self.target_files_dir, name)) as f:
      return f.read()

  def test_set_release_tags(self):
    self.write("SYSTEM/build.prop",
               "ro.build.tags=test-keys\n"
               "ro.system.build.fingerprint=b/p/d:17/ID/1:user/test-keys\n")
    self.write("SYSTEM/etc/tags.txt", "ro.build.tags=test-keys\n")
    self.write("META/misc_info.txt",
               "avb_enable=true\n"
               "avb_system_add_hashtree_footer_args=--prop "
               "com.android.build.system.fingerprint:b/p/d:17/ID/1:user/test-keys\n")
    # A link out of the directory, which must not change
    os.makedirs(os.path.join(self.target_files_dir, "BOOT/RAMDISK"))
    os.symlink("/nonexistent/prop.default",
               os.path.join(self.target_files_dir, "BOOT/RAMDISK/prop.default"))

    set_release_tags(self.target_files_dir)

    self.assertEqual(
        "ro.build.tags=release-keys\n"
        "ro.system.build.fingerprint=b/p/d:17/ID/1:user/release-keys\n\n",
        self.read("SYSTEM/build.prop"))
    self.assertEqual("ro.build.tags=test-keys\n",
                     self.read("SYSTEM/etc/tags.txt"))
    self.assertEqual(
        "avb_enable=true\n"
        "avb_system_add_hashtree_footer_args=--prop "
        "com.android.build.system.fingerprint:b/p/d:17/ID/1:user/release-keys\n",
        self.read("META/misc_info.txt"))
