override GNUMAKEFLAGS = -j3 READY:=yes STATE:=$$(if\ $$(value\ GNUMAKEFLAGS),pending,cleared) MARK:=$$(file\ >>seen,GNU)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)),$(file >>seen,FLAGS) -j1)
