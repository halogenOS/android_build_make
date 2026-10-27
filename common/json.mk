4space :=$= $(space)$(space)$(space)$(space)
invert_bool =$= $(if $(strip $(1)),,true)

# Converts a list to a JSON list.
# $1: List separator.
# $2: List.
_json_list =$= [$(if $(2),"$(subst $(1),"$(comma)",$(2))")]

# Converts a space-separated list to a JSON list.
json_list =$= $(call _json_list,$(space),$(1))

# Converts a comma-separated list to a JSON list.
csv_to_json_list =$= $(call _json_list,$(comma),$(1))

# Returns the text without surrounding spaces as a JSON string with every
# character escaped that JSON requires, or nothing for empty text.
# $1: Text.
json-string =$= $(if $(strip $(1)),$(KATI_json_quote $(strip $(1))))

# Returns the words as a JSON array of strings escaped like json-string, or
# nothing for no words.
# $1: Words.
json-strings =$= $(if $(strip $(1)),[$(KATI_foreach_sep w,$(comma),$(1),$(KATI_json_quote $(w)))])

# Returns a comma and the member of a JSON object, or nothing for no value.
# $1: Key name.
# $2: JSON value, or nothing.
json-member =$= $(if $(2),$(comma)"$(1)":$(2))

# Adds or removes 4 spaces from _json_indent
json_increase_indent =$= $(eval _json_indent := $$(_json_indent)$$(4space))
json_decrease_indent =$= $(eval _json_indent := $$(subst _,$$(space),$$(patsubst %____,%,$$(subst $$(space),_,$$(_json_indent)))))

# 1: Key name
# 2: Value
add_json_val =$= $(eval _json_contents := $$(_json_contents)$$(_json_indent)"$$(strip $$(1))": $$(strip $$(2))$$(comma)$$(newline))
add_json_str =$= $(call add_json_val,$(1),"$(strip $(2))")
add_json_list =$= $(call add_json_val,$(1),$(call json_list,$(patsubst %,%,$(2))))
add_json_csv =$= $(call add_json_val,$(1),$(call csv_to_json_list,$(strip $(2))))
add_json_bool =$= $(call add_json_val,$(1),$(if $(strip $(2)),true,false))
add_json_map =$= $(eval _json_contents := $$(_json_contents)$$(_json_indent)"$$(strip $$(1))": {$$(newline))$(json_increase_indent)
add_json_map_anon =$= $(eval _json_contents := $$(_json_contents)$$(_json_indent){$$(newline))$(json_increase_indent)
end_json_map =$= $(json_decrease_indent)$(eval _json_contents := $$(_json_contents)$$(if $$(filter %$$(comma),$$(lastword $$(_json_contents))),__SV_END)$$(_json_indent)},$$(newline))
add_json_array =$= $(eval _json_contents := $$(_json_contents)$$(_json_indent)"$$(strip $$(1))": [$$(newline))$(json_increase_indent)
end_json_array =$= $(json_decrease_indent)$(eval _json_contents := $$(_json_contents)$$(if $$(filter %$$(comma),$$(lastword $$(_json_contents))),__SV_END)$$(_json_indent)],$$(newline))

# Clears _json_contents to start a new json file
json_start =$= $(eval _json_contents := {$$(newline))$(eval _json_indent := $$(4space))

# Adds the trailing close brace to _json_contents, and removes any trailing commas if necessary
json_end =$= $(eval _json_contents := $$(subst $$(comma)$$(newline)__SV_END,$$(newline),$$(_json_contents)__SV_END}$$(newline)))

json_contents =$= $(_json_contents)
