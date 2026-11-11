/^# Implicit Rules$/ { section = "implicit"; print section }
/^# Files$/ { section = "files"; print section }
/^# files hash-table stats:/ { section = "" }
section == "files" && /^[.]RECIPEPREFIX = / { print }
section != "" && /^[\t>!]/ && /-marker/ { print }
