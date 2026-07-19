# pam-to-png

Independent, zero-dependency Rust conversion from Netpbm PAM to Portable Network Graphics. PNG parsing covers all legal color types and depths, all Deflate block types, five filters and Adam7. A 16-bit PNG source requires the explicit `allow_sample_scaling` option when targeting an 8-bit carrier.
