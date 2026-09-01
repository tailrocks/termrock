export const TERMROCK_GIT_URL = 'https://github.com/tailrocks/termrock.git'
export const TERMROCK_REVIEWED_REVISION =
  '5283c2acf9154d0cfcd37b1ffe821c00faf90ea2'
export const TERMROCK_INSTALL_COMMAND =
  `cargo add termrock --git ${TERMROCK_GIT_URL} --rev ${TERMROCK_REVIEWED_REVISION}`
export const TERMROCK_CARGO_DEPENDENCY =
  `termrock = { git = "${TERMROCK_GIT_URL}", rev = "${TERMROCK_REVIEWED_REVISION}" }`
export const TERMROCK_CROSSTERM_DEPENDENCY =
  `termrock = { git = "${TERMROCK_GIT_URL}", rev = "${TERMROCK_REVIEWED_REVISION}", features = ["crossterm"] }`
