local ffi = require("ffi")
local bit = require("bit")

local sng = {}

-- Trying FFI for performance
ffi.cdef[[
    typedef struct {
        uint8_t data[256];
    } lookup_table_t;
]]

local FILE_IDENTIFIER = "SNGPKG"

-- Helper functions for reading binary data (Def not skidded)
local function readBytes(file, count)
    local data = file:read(count)
    if not data or #data < count then
        error("Unexpected end of file")
    end
    return data
end

local function readUInt32LE(file)
    local bytes = readBytes(file, 4)
    local b1, b2, b3, b4 = bytes:byte(1, 4)
    return b1 + b2 * 256 + b3 * 65536 + b4 * 16777216
end

local function readUInt64LE(file)
    local bytes = readBytes(file, 8)
    local b1, b2, b3, b4, b5, b6, b7, b8 = bytes:byte(1, 8)

    local low = b1 + b2 * 256 + b3 * 65536 + b4 * 16777216
    local high = b5 + b6 * 256 + b7 * 65536 + b8 * 16777216
    return low + high * 4294967296
end

local function readInt32LE(file)
    local value = readUInt32LE(file)
    if value >= 0x80000000 then
        value = value - 0x100000000
    end
    return value
end

local function readString(file, length)
    if length == 0 then return "" end
    local bytes = readBytes(file, length)
    return bytes
end

local function maskData(data, seed)
    local result = {}
    
    for i = 1, #data do
        local idx = i - 1  -- Convert to 0-based for algorithm
        local seedIdx = (idx % 16) + 1
        local xorKey = bit.bxor(seed[seedIdx], bit.band(idx, 0xFF))
        result[i] = bit.bxor(data:byte(i), xorKey)
    end
    
    return string.char(unpack(result))
end

local function maskDataFast(data, seed)
    local len = #data
    local result = ffi.new("uint8_t[?]", len)
    
    -- Correct algorithm from SNG documentation:
    -- xorKey = xorMask[i % 16] XOR (i AND 0xFF)
    -- fileBytes[i] = maskedFileBytes[i] XOR xorKey
    
    for i = 0, len - 1 do
        local seedIdx = (i % 16) + 1
        local xorKey = bit.bxor(seed[seedIdx], bit.band(i, 0xFF))
        result[i] = bit.bxor(data:byte(i + 1), xorKey)
    end
    
    return ffi.string(result, len)
end

-- Main SNG File structure
local SngFile = {}
SngFile.__index = SngFile

function SngFile.new()
    local self = setmetatable({}, SngFile)
    self.version = 0
    self.xorMask = {}
    self.metadata = {}
    self.files = {}
    return self
end

function SngFile:setMetadata(key, value)
    self.metadata[key] = value
end

function SngFile:addFile(name, contents)
    self.files[name] = contents
end

function SngFile:getFile(name)
    return self.files[name]
end

function SngFile:getAllFiles()
    return self.files
end

-- Load SNG File
function sng.load(filepath)
    local file = io.open(filepath, "rb")
    if not file then
        error("Could not open file: " .. filepath)
    end
    
    local sngFile = SngFile.new()
    
    -- Read and verify file identifier
    local identifier = readBytes(file, 6)
    if identifier ~= FILE_IDENTIFIER then
        file:close()
        error("Invalid SNG file identifier. Expected: " .. FILE_IDENTIFIER .. ", got: " .. identifier)
    end
    
    -- Read version
    sngFile.version = readUInt32LE(file)
    
    -- Read XOR mask (16 bytes)
    local maskBytes = readBytes(file, 16)
    for i = 1, 16 do
        sngFile.xorMask[i] = maskBytes:byte(i)
    end
    
    -- Read metadata section
    local metadataLen = readUInt64LE(file)
    local metadataCount = readUInt64LE(file)
    
    for i = 1, metadataCount do
        local keyLen = readInt32LE(file)
        if keyLen < 0 then
            file:close()
            error("Metadata key length cannot be negative")
        end
        
        local key = readString(file, keyLen)
        
        local valueLen = readInt32LE(file)
        if valueLen < 0 then
            file:close()
            error("Metadata value length cannot be negative")
        end
        
        local value = readString(file, valueLen)
        sngFile:setMetadata(key, value)
    end
    
    -- Read file index
    local fileIndexLen = readUInt64LE(file)
    local fileCount = readUInt64LE(file)
    
    local fileInfo = {}
    for i = 1, fileCount do
        local fileNameLength = file:read(1):byte()
        local fileName = readString(file, fileNameLength)
        local contentsLen = readUInt64LE(file)
        local contentsIndex = readUInt64LE(file)
        
        table.insert(fileInfo, {
            index = contentsIndex,
            size = contentsLen,
            name = fileName
        })
    end
    
    -- Read file section length
    local filesSectionLen = readUInt64LE(file)
    
    -- DEBUG: Check current position
    local currentFilePos = file:seek()
    print("\n=== FILE POSITION DEBUG ===")
    print("Current position after reading all headers:", currentFilePos)
    print("File data section length:", filesSectionLen)
    print("Files should start at position:", currentFilePos)
    
    -- Show where each file claims to be vs where they should be
    print("\nFile position analysis:")
    for i, info in ipairs(fileInfo) do
        local expectedPos = currentFilePos + (i == 1 and 0 or 
            (fileInfo[i-1].index - currentFilePos + fileInfo[i-1].size))
        print(string.format("  %s: stored_index=%d, expected_if_sequential=%d, diff=%d",
            info.name, info.index, expectedPos, info.index - expectedPos))
    end
    print("===========================\n")
    
    -- Read and decrypt file contents
    for _, info in ipairs(fileInfo) do
        -- Seek to file position
        file:seek("set", info.index)
        
        print("DEBUG DECRYPT:", info.name)
        print("  File position (index):", info.index)
        print("  File size:", info.size)
        
        -- Read encrypted data
        local encryptedData = readBytes(file, info.size)
        
        -- Print first 16 bytes of encrypted data
        local hexStr = ""
        for i = 1, math.min(16, #encryptedData) do
            hexStr = hexStr .. string.format("%02X ", encryptedData:byte(i))
        end
        print("  Encrypted (first 16 bytes):", hexStr)
        
        -- Print XOR mask
        local maskStr = ""
        for i = 1, 16 do
            maskStr = maskStr .. string.format("%02X ", sngFile.xorMask[i])
        end
        print("  XOR Mask:", maskStr)
        
        -- Manual calculation for first byte
        local firstByte = encryptedData:byte(1)
        local pos = info.index
        local lookupIdx = pos % 256
        local seedIdx = (lookupIdx % 16) + 1  -- +1 for Lua
        local lookupVal = bit.bxor(lookupIdx, sngFile.xorMask[seedIdx])
        local decrypted = bit.bxor(firstByte, lookupVal)
        print(string.format("  First byte trace: pos=%d, pos%%256=%d, seedIdx=%d, seed=0x%02X, lookup=0x%02X, encrypted=0x%02X, decrypted=0x%02X",
            pos, lookupIdx, seedIdx, sngFile.xorMask[seedIdx], lookupVal, firstByte, decrypted))
        
        -- Decrypt using XOR mask
        local decryptedData = maskDataFast(encryptedData, sngFile.xorMask)
        
        -- Print first 16 bytes of decrypted data
        hexStr = ""
        for i = 1, math.min(16, #decryptedData) do
            hexStr = hexStr .. string.format("%02X ", decryptedData:byte(i))
        end
        print("  Decrypted (first 16 bytes):", hexStr)
        
        -- Verify file format
        if info.name:match("%.jpe?g$") then
            local b1, b2 = decryptedData:byte(1, 2)
            if b1 == 0xFF and b2 == 0xD8 then
                print(" Valid JPEG header")
            else
                print(" Invalid JPEG header! Expected FF D8, got", string.format("%02X %02X", b1, b2))
            end
        elseif info.name:match("%.opus$") then
            local header = decryptedData:sub(1, 8)
            if header == "OpusHead" then
                print("  Valid Opus header")
            else
                print("  Invalid Opus header! Got:", header:sub(1, 8))
            end
        end
        
        sngFile:addFile(info.name, decryptedData)
    end
    
    file:close()
    return sngFile
end

local function createDirectoryRecursive(path)
    if love and love.filesystem then
        local saveDir = love.filesystem.getSaveDirectory()
        if path:sub(1, #saveDir) == saveDir then
            local relativePath = path:sub(#saveDir + 2)
            
            -- Create all parent directories
            local parts = {}
            for part in relativePath:gmatch("[^/\\]+") do
                table.insert(parts, part)
            end
            
            local currentPath = ""
            for i = 1, #parts do
                if currentPath == "" then
                    currentPath = parts[i]
                else
                    currentPath = currentPath .. "/" .. parts[i]
                end
                
                if not love.filesystem.getInfo(currentPath) then
                    local success = love.filesystem.createDirectory(currentPath)
                    if not success then
                        print("Warning: Could not create directory:", currentPath)
                    end
                end
            end
            return true
        end
    end
    
    -- Fallback to os command (Windows/Unix compatible)
    local separator = package.config:sub(1,1) == "\\" and "\\" or "/"
    local cmd
    if separator == "\\" then
        cmd = 'mkdir "' .. path:gsub("/", "\\") .. '" 2>nul'
    else
        cmd = 'mkdir -p "' .. path .. '"'
    end
    os.execute(cmd)
    return true
end

function sng.extractToDirectory(sngFile, outputPath)
    createDirectoryRecursive(outputPath)
    
    local saveDir = love and love.filesystem and love.filesystem.getSaveDirectory()
    local useRelativePath = saveDir and outputPath:sub(1, #saveDir) == saveDir
    local relativePath = useRelativePath and outputPath:sub(#saveDir + 2) or outputPath
    
    -- Save metadata as song.ini
    local iniContent = "[song]\n"
    for key, value in pairs(sngFile.metadata) do
        iniContent = iniContent .. key .. " = " .. value .. "\n"
    end
    
    if useRelativePath then
        love.filesystem.write(relativePath .. "/song.ini", iniContent)
    else
        local file = io.open(outputPath .. "/song.ini", "wb")
        if file then
            file:write(iniContent)
            file:close()
        end
    end
    
    for filename, contents in pairs(sngFile.files) do
        -- Handle subdirectories in filename
        local parts = {}
        for part in filename:gmatch("[^/]+") do
            table.insert(parts, part)
        end
        
        if #parts > 1 then
            -- Create subdirectories
            if useRelativePath then
                local dirPath = relativePath
                for i = 1, #parts - 1 do
                    dirPath = dirPath .. "/" .. parts[i]
                    if not love.filesystem.getInfo(dirPath) then
                        love.filesystem.createDirectory(dirPath)
                    end
                end
            else
                local dirPath = outputPath
                for i = 1, #parts - 1 do
                    dirPath = dirPath .. "/" .. parts[i]
                    createDirectoryRecursive(dirPath)
                end
            end
        end
        
        -- Write file
        if useRelativePath then
            love.filesystem.write(relativePath .. "/" .. filename, contents)
        else
            local file = io.open(outputPath .. "/" .. filename, "wb")
            if file then
                file:write(contents)
                file:close()
            end
        end
    end
end

function sng.loadFromLove(filepath)
    local data = love.filesystem.read(filepath)
    if not data then
        error("Could not read file: " .. filepath)
    end
    
    -- Create temporary file for processing
    local tempPath = os.tmpname()
    local tempFile = io.open(tempPath, "wb")
    tempFile:write(data)
    tempFile:close()
    
    local sngFile = sng.load(tempPath)
    os.remove(tempPath)
    
    return sngFile
end

function sng.findAllSngFiles(directory)
    local files = {}
    
    local function scanDirectory(dir)
        local items = love.filesystem.getDirectoryItems(dir)
        for _, item in ipairs(items) do
            local path = dir .. "/" .. item
            local info = love.filesystem.getInfo(path)
            
            if info.type == "directory" then
                scanDirectory(path)
            elseif info.type == "file" and path:match("%.sng$") then
                table.insert(files, path)
            end
        end
    end
    
    scanDirectory(directory)
    return files
end

return sng