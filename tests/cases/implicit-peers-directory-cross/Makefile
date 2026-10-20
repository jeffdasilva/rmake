all: dir/x.a foo/dir/x.b
%.a foo/%.b:;@echo "$@:$*";mkdir -p foo/dir;touch dir/x.a foo/dir/x.b
