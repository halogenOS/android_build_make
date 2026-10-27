# SPDX-FileCopyrightText: The halogenOS Project
# SPDX-License-Identifier: Apache-2.0

# Writes the compliance metadata of Make as JSON lines, one line per installed
# file into make_metadata.jsonl and one per Make module into make_modules.jsonl.
# The first line of each file is the header line of the file of the same name in
# compliance_metadata/, which lists the columns of the table. Soong reads the
# same header lines in a build without Kati. Soong imports the files into the
# compliance metadata database.
#
# Included at the end of main.mk, which sets these first:
#   installed_files: the installed files of the product
#   _make_modules: the modules that are defined in Make

# Precompute these as an optimization to not do $(findstring).
# Normally we would unset these to save memory, but we're almost at the end of the make
# run, so don't bother.
$(foreach f,$(INSTALLED_PRODUCT_SYSTEM_OTHER_AVBKEY_TARGET),\
	$(eval _is_product_system_other_avbkey.$(f):=Y) \
)
$(foreach f,$(event_log_tags_file),\
	$(eval _is_event_log_tags_file.$(f):=Y) \
)
$(foreach f,$(INSTALLED_SYSTEM_OTHER_ODEX_MARKER),\
	$(eval _is_system_other_odex_marker.$(f):=Y) \
)
$(foreach f,$(ALL_KERNEL_MODULES_BLOCKLIST),\
	$(eval _is_kernel_modules_blocklist.$(f):=Y) \
)
$(foreach f,$(ALL_FSVERITY_BUILD_MANIFEST_APK),\
	$(eval _is_fsverity_build_manifest_apk.$(f):=Y) \
)
$(foreach f,$(SYSTEM_LINKER_CONFIG),\
	$(eval _is_linker_config.$(f):=Y) \
)
$(foreach f,$(vendor_linker_config_file),\
	$(eval _is_linker_config.$(f):=Y) \
)
$(foreach f,$(product_linker_config_file),\
	$(eval _is_linker_config.$(f):=Y) \
)
$(foreach f,$(PARTITION_COMPAT_SYMLINKS),\
	$(eval _is_partition_compat_symlink.$(f):=Y) \
)
$(foreach f,$(ALL_FLAGS_FILES),\
	$(eval _is_flags_file.$(f):=Y) \
)
$(foreach f,$(ALL_ROOTDIR_SYMLINKS),\
	$(eval _is_rootdir_symlink.$(f):=Y) \
)
$(foreach m,$(ALL_NON_MODULES),$(eval _is_non_module.$(m):=Y))

# $(1): an installed file
# $(2): the module that installs it
# $(3): the path of the file below PRODUCT_OUT
define make-metadata-record
{"installed_file":$(call json-paths,$(1))
$(call json-member,module_path,$(call json-paths,$(sort $(ALL_MODULES.$(2).PATH))))
$(call json-member,is_soong_module,$(call json-string,$(ALL_MODULES.$(2).IS_SOONG_MODULE)))
$(call json-member,is_prebuilt_make_module,$(call json-string,$(ALL_MODULES.$(2).IS_PREBUILT_MAKE_MODULE)))
$(call json-member,product_copy_files,$(call json-strings,$(sort $(filter %:$(3),$(product_copy_files_without_owner)))))
$(call json-member,kernel_module_copy_files,$(call json-strings,$(sort $(filter %$(3),$(KERNEL_MODULE_COPY_FILES)))))
$(call json-member,is_platform_generated,$(call json-string,$(if $(ALL_MODULES.$(2).IS_SOONG_MODULE),,$(call is-build-prop,$(1))$(_is_product_system_other_avbkey.$(1))$(_is_event_log_tags_file.$(1))$(_is_system_other_odex_marker.$(1))$(_is_kernel_modules_blocklist.$(1))$(_is_fsverity_build_manifest_apk.$(1))$(_is_linker_config.$(1))$(_is_partition_compat_symlink.$(1))$(_is_flags_file.$(1))$(_is_rootdir_symlink.$(1)))))
$(call json-member,static_libs,$(call json-strings,$(if $(ALL_MODULES.$(2).IS_SOONG_MODULE),,$(ALL_INSTALLED_FILES.$(1).STATIC_LIBRARIES))))
$(call json-member,whole_static_libs,$(call json-strings,$(if $(ALL_MODULES.$(2).IS_SOONG_MODULE),,$(ALL_INSTALLED_FILES.$(1).WHOLE_STATIC_LIBRARIES))))
$(call json-member,license_text,$(call json-paths,$(if $(_is_non_module.$(1)),$(ALL_NON_MODULES.$(1).NOTICES),$(if $(_is_partition_compat_symlink.$(1)),build/soong/licenses/LICENSE,$(ALL_MODULES.$(2).NOTICES)))))}
endef

# $(1): a module defined in Make
define make-modules-record
{"name":$(call json-string,$(1))
$(call json-member,module_path,$(call json-paths,$(sort $(ALL_MODULES.$(1).PATH))))
$(call json-member,module_class,$(call json-string,$(ALL_MODULES.$(1).CLASS)))
$(call json-member,module_type,$(call json-string,$(ALL_MODULES.$(1).MAKE_MODULE_TYPE)))
$(call json-member,static_libs,$(call json-strings,$(sort $(ALL_MODULES.$(1).STATIC_LIBS))))
$(call json-member,whole_static_libs,$(call json-strings,$(sort $(ALL_MODULES.$(1).WHOLE_STATIC_LIBS))))
$(call json-member,built_files,$(call json-paths,$(sort $(ALL_MODULES.$(1).BUILT))))
$(call json-member,installed_files,$(call json-paths,$(sort $(ALL_MODULES.$(1).INSTALLED))))}
endef

_compliance_metadata_dir := $(SOONG_OUT_DIR)/compliance-metadata/$(TARGET_PRODUCT)
_make_metadata_jsonl := $(_compliance_metadata_dir)/make_metadata.jsonl
_make_modules_jsonl := $(_compliance_metadata_dir)/make_modules.jsonl

$(shell mkdir -p $(_compliance_metadata_dir))

$(call write-json-line,$(_make_metadata_jsonl),$(file <$(BUILD_SYSTEM)/compliance_metadata/make_metadata.jsonl))
$(foreach f,$(installed_files),\
  $(call write-json-line,$(_make_metadata_jsonl),$(call make-metadata-record,$(f),$(ALL_INSTALLED_FILES.$(f)),$(patsubst $(PRODUCT_OUT)/%,%,$(f)))))

$(call write-json-line,$(_make_modules_jsonl),$(file <$(BUILD_SYSTEM)/compliance_metadata/make_modules.jsonl))
$(foreach m,$(_make_modules),$(call write-json-line,$(_make_modules_jsonl),$(call make-modules-record,$(m))))
