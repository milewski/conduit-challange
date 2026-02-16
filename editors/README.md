# Editor Support

## JetBrains (TextMate)

Conduit ships with a TextMate bundle at:

`editors/textmate`

To enable highlighting for `.conduit` files in JetBrains IDEs:

1. Open **Settings** → **Editor** → **TextMate Bundles**
2. Click **+** and select the folder `editors/textmate` (the folder with `package.json`)
3. Open **Settings** → **Editor** → **File Types**
4. Add `*.conduit` to the TextMate file type if it is not auto-detected

Grammar file path inside the bundle:

`editors/textmate/syntaxes/conduit.tmLanguage.json`

This bundle is compatible with JetBrains TextMate import and can also be reused by other editors that support TextMate grammars.
