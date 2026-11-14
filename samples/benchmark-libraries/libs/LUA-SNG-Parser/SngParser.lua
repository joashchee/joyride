local sng = require("libs.sng")

local SngParser = {}

-- Returns: success (boolean), error (string or nil)
function SngParser.extractSng(sngPath, outputPath)
    local success, result = pcall(function()
        print("SngParser: Loading SNG from:", sngPath)
        
        -- Load the SNG file
        local sngFile = sng.load(sngPath)
        
        print("SngParser: Loaded version", sngFile.version)
        
        -- Count files in package
        local fileCount = 0
        for _ in pairs(sngFile.files) do
            fileCount = fileCount + 1
        end
        print("SngParser: Files in package:", fileCount)
        
        -- Extract to target directory
        print("SngParser: Extracting to:", outputPath)
        sng.extractToDirectory(sngFile, outputPath)
        
        print("SngParser: Extraction complete!")
        return true
    end)
    
    if success then
        return true, nil
    else
        print("SngParser ERROR:", result)
        return false, tostring(result)
    end
end

-- Returns: sngFile object or nil, error
function SngParser.loadSng(sngPath)
    local success, result = pcall(function()
        return sng.load(sngPath)
    end)
    
    if success then
        return result, nil
    else
        return nil, tostring(result)
    end
end

-- Returns: metadata table or nil, error
function SngParser.getMetadata(sngPath)
    local sngFile, err = SngParser.loadSng(sngPath)
    if not sngFile then
        return nil, err
    end
    
    return sngFile.metadata, nil
end

-- Returns: boolean, error message if invalid
function SngParser.isValidSng(sngPath)
    local file = io.open(sngPath, "rb")
    if not file then
        return false, "Cannot open file"
    end
    
    local identifier = file:read(6)
    file:close()
    
    if identifier ~= "SNGPKG" then
        return false, "Invalid SNG identifier"
    end
    
    return true, nil
end

-- Returns: array of filenames or nil, error
function SngParser.listFiles(sngPath)
    local sngFile, err = SngParser.loadSng(sngPath)
    if not sngFile then
        return nil, err
    end
    
    local fileList = {}
    for filename, _ in pairs(sngFile.files) do
        table.insert(fileList, filename)
    end
    
    return fileList, nil
end

-- Returns: file contents or nil, error
function SngParser.extractFile(sngPath, filename)
    local sngFile, err = SngParser.loadSng(sngPath)
    if not sngFile then
        return nil, err
    end
    
    local contents = sngFile:getFile(filename)
    if not contents then
        return nil, "File not found in SNG: " .. filename
    end
    
    return contents, nil
end

-- callback(index, total, filename, status, error)
-- status can be: "processing", "success", "error"
function SngParser.batchExtract(sngPaths, outputBasePath, callback)
    local stats = {
        total = #sngPaths,
        success = 0,
        failed = 0,
        errors = {}
    }
    
    for i, sngPath in ipairs(sngPaths) do
        local filename = sngPath:match("([^/\\]+)%.sng$") or "unknown"
        local outputPath = outputBasePath .. "/" .. filename:gsub("%.sng$", "")
        
        if callback then
            callback(i, stats.total, filename, "processing", nil)
        end
        
        local success, err = SngParser.extractSng(sngPath, outputPath)
        
        if success then
            stats.success = stats.success + 1
            if callback then
                callback(i, stats.total, filename, "success", nil)
            end
        else
            stats.failed = stats.failed + 1
            table.insert(stats.errors, {file = sngPath, error = err})
            if callback then
                callback(i, stats.total, filename, "error", err)
            end
        end
    end
    
    return stats
end

function SngParser.findSngFiles(directory)
    return sng.findAllSngFiles(directory)
end

return SngParser