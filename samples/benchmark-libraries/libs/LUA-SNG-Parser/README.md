# SNG Parser

A Lua library for parsing and extracting SNG package files. Built for integration with LÖVE2D/LuaJIT applications.

## What is SNG?

SNG (presumably "Song Package") is a custom binary archive format that bundles files with metadata. Files within the archive are XOR-encrypted using a position-based masking scheme.

## Credits

This implementation is based on the SNG file format specification from [mdsitton/SngFileFormat](https://github.com/mdsitton/SngFileFormat).

## Installation

Copy `sng.lua` and `SngParser.lua` to your project's lib directory.
```lua
local SngParser = require("SngParser")
```

## Usage

### Basic Extraction
```lua
local success, err = SngParser.extractSng("path/to/file.sng", "output/directory")
if not success then
    print("Extraction failed:", err)
end
```

### Read Metadata Without Extracting
```lua
local metadata, err = SngParser.getMetadata("path/to/file.sng")
if metadata then
    for key, value in pairs(metadata) do
        print(key, value)
    end
end
```

### List Files in Archive
```lua
local files, err = SngParser.listFiles("path/to/file.sng")
if files then
    for _, filename in ipairs(files) do
        print(filename)
    end
end
```

### Extract Single File
```lua
local contents, err = SngParser.extractFile("path/to/file.sng", "audio.opus")
if contents then
    -- Do something with file contents
end
```

### Batch Processing
```lua
local sngFiles = SngParser.findSngFiles("songs/")

local stats = SngParser.batchExtract(sngFiles, "output/", function(index, total, filename, status, error)
    if status == "processing" then
        print(string.format("[%d/%d] Processing %s...", index, total, filename))
    elseif status == "error" then
        print(string.format("[%d/%d] Failed: %s - %s", index, total, filename, error))
    end
end)

print(string.format("Completed: %d successful, %d failed", stats.success, stats.failed))
```

## File Format Specification

### Header Structure

- **Identifier**: 6 bytes - "SNGPKG"
- **Version**: 4 bytes (uint32 LE)
- **XOR Mask**: 16 bytes

### Metadata Section

- **Section Length**: 8 bytes (uint64 LE)
- **Entry Count**: 8 bytes (uint64 LE)
- For each entry:
  - Key length (4 bytes, int32 LE)
  - Key string
  - Value length (4 bytes, int32 LE)
  - Value string

### File Index Section

- **Section Length**: 8 bytes (uint64 LE)
- **File Count**: 8 bytes (uint64 LE)
- For each file:
  - Filename length (1 byte)
  - Filename string
  - Content length (8 bytes, uint64 LE)
  - Content offset (8 bytes, uint64 LE)

### File Data Section

- **Section Length**: 8 bytes (uint64 LE)
- XOR-encrypted file contents

### Decryption Algorithm
```
xorKey = xorMask[position % 16] XOR (position AND 0xFF)
decryptedByte = encryptedByte XOR xorKey
```

## API Reference

### SngParser

**extractSng(sngPath, outputPath)**  
Extracts all files and generates a `song.ini` from metadata.  
Returns: `success (boolean), error (string or nil)`

**loadSng(sngPath)**  
Loads an SNG file into memory without extracting.  
Returns: `sngFile object or nil, error`

**getMetadata(sngPath)**  
Returns metadata table from the archive.  
Returns: `metadata table or nil, error`

**isValidSng(sngPath)**  
Checks if a file has a valid SNG header.  
Returns: `boolean, error message`

**listFiles(sngPath)**  
Returns array of all filenames in the archive.  
Returns: `array or nil, error`

**extractFile(sngPath, filename)**  
Extracts a single file's contents.  
Returns: `contents or nil, error`

**batchExtract(sngPaths, outputBasePath, callback)**  
Processes multiple SNG files with progress tracking.  
Returns: `stats table {total, success, failed, errors}`

**findSngFiles(directory)**  
Recursively scans a directory for .sng files.  
Returns: `array of paths`

### Low-level API (sng.lua)

The underlying `sng` module provides direct access to the parser if needed:
```lua
local sng = require("libs.sng")
local sngFile = sng.load("file.sng")
local contents = sngFile:getFile("audio.opus")
```

## Dependencies

- LuaJIT FFI (for optimized decryption)
- bit library (bitwise operations)
- LÖVE2D filesystem (optional, for certain operations)

## Notes

The parser includes debug output during file decryption. This is currently enabled and will print hex dumps of encrypted/decrypted data to help verify the decryption process works correctly.
