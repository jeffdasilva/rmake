$(shell touch -d @1600000000 input second; touch -d @1600000001 cached)
cached:: input ; @: >cached
cached:: second ; @: >cached
work: ; @$(info expanded-work)printf 'worked\n'
