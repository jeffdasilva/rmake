# Roomy jobserver wake slots

This finite glibc profile uses a hard descriptor limit of at least 64.
The controller starts each closed-auth invocation with only standard
descriptors. Both 16,17 and 17,16 must produce the literal fallback warning
and a serial recipe with no advertised pool. The candidate's actual live
slots 16 and 17 must remain the same private nonblocking close-on-exec pipe.

The valid control seeds one byte in a real pipe, passes two inherited alias
pairs at 16/17 and 18/19, and advertises the second pair. The single recipe uses
the implicit slot. The live report verifies all four incoming physical
identities, their access modes, the queued byte count and the inheritance
policy of the selected and unadvertised aliases. The candidate's private wake
pair uses 20/21 in this control, and neither end reaches the recipe.

The runner retains full observed descriptor flags. GNU marks the inherited
read description nonblocking in this control. rmake uses a private
nonblocking reader and leaves the inherited read aliases blocking. That
separate observable remains open. These models cover wake-slot isolation,
literal output and status, the complete fixture filesystem, and reaping.

Run the development gate with `make test-roomy-wake-slots` and the optimized
gate with `make test-roomy-wake-slots-release`. Both also run the existing
shell capture fixture with its explicit roomy numeric checks.
