MAKEOVERRIDES=invalid.name=new
all:
	@$(info make=<$(value invalid.name)>|$(origin invalid.name))python3 -c 'import os; print(os.environ.get("invalid.name", "absent"))'
