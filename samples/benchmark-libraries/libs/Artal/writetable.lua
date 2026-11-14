--[[
The MIT License (MIT)

Copyright (c) 2016 Daniel Rasmussen

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
--]]

local writetable = {}
writetable.version = "1.0"
local recursivething --Forward declare function.

local function indent(num)
	return string.rep("\t",num)
end

local function keyValueErrorMessage(k,v)
	return "\nKey Name: "..tostring(k).."\nKey Type: "..type(k).."\nValue Name: "..tostring(v).."\nValue Type: "..type(v)
end

local function appendKey(k,v,recurseLevel)
	local result = ""

	result = result..indent(recurseLevel)
	if type(k) == "number" then
		result = result.."["..k.."]"
	elseif type(k) == "string" then
		result = result..k
	else
		assert(false, "Unhandled type of key."..keyValueErrorMessage(k,v))
	end

	return result
end

local function appendValue(k,v,recurseLevel)
	local result = ""

	if type(v) == "number" or type(v) == "boolean" then
		result = result..tostring(v)
	elseif type(v) == "string" then
		result = result.."\""..tostring(v).."\""
	elseif type(v) == "table" then
		result = result.."\n"..recursivething(v,recurseLevel)
	elseif type(v) == "userdata" or type(v) == "function" then 
		result = result.."\""..tostring(v).."\""
	else
		assert(false, "Unhandled type of Value."..keyValueErrorMessage(k,v))
	end
	result = result..",\n"

	return result
end


function recursivething(inTable,recurseLevel) -- This function is forward declared, not global.
	local result = ""
	local numberOfKeys = 0
	local stringKeys = 0
	local indexKeys = 0
	local numberOfTableValue = 0
	local maxIndex = 0
	local isContiguous = true
	local contiguousTo = 0
	local arrayValueType = nil

	recurseLevel = recurseLevel + 1
	result = result..indent(recurseLevel-1).."{\n"

	do -- Analyse table structure.
		for k, v in pairs(inTable) do
			if type(k) == "number" then
				maxIndex = math.max(maxIndex, k)
				indexKeys = indexKeys + 1

				if arrayValueType == nil then
					arrayValueType = type(v)
				elseif arrayValueType ~= type(v) then
					arrayValueType = false
				end
			end
			if type(k) == "string" then
				stringKeys = stringKeys + 1
			end

			if type(v) == "table" then
				numberOfTableValue = numberOfTableValue + 1
			end
			numberOfKeys = numberOfKeys + 1
		end
		if indexKeys > 0 then
			for i = 1, maxIndex do
				if inTable[i] ~= nil then
					contiguousTo = i
				else
					isContiguous = false
					break
				end
			end
		end

		if numberOfKeys > 0 then
			if numberOfKeys == indexKeys and numberOfKeys == contiguousTo then
				-- pure index table, contiguous
				result = result..indent(recurseLevel).."-- Array with "..indexKeys.." indexes.\n"
			elseif numberOfKeys == indexKeys then
				-- pure index table, NOT contiguous
				result = result..indent(recurseLevel).."-- Non contiguous array with "..indexKeys.." indexes.\n"
			elseif numberOfKeys == stringKeys then
				-- pure string key table
				result = result..indent(recurseLevel).."-- Table with "..stringKeys.." string keys.\n"
			elseif numberOfKeys == (stringKeys + contiguousTo) then
				-- string / index table. contiguous
				result = result..indent(recurseLevel).."-- Table with "..indexKeys.." indexes, "
				result = result.."and "..stringKeys.." string keys.\n"
			else
				-- string / index table. NOT contiguous
				result = result..indent(recurseLevel).."-- Non contiguous table with "..indexKeys.." indexes, "
				result = result.."and "..stringKeys.." string keys.\n"
			end
			if arrayValueType then
				result = result..indent(recurseLevel).."-- Array values are all of type: \""..arrayValueType.."\".\n"
			end
			if numberOfTableValue > 0 and (arrayValueType ~= "table" or numberOfTableValue ~= indexKeys)  then
				result = result..indent(recurseLevel).."-- Table has "..numberOfTableValue.." sub-tables.\n"
			end
			
		else
			result = result..indent(recurseLevel).."-- Empty Table.\n"
		end
	end

	do -- Build the contents of the table to string.
		-- String keys goes first.
		-- Keys with table value is defered to the next loop.
		-- Index keys is defered to the last loop.
		local keysWithTables = {}
		for k, v in pairs(inTable) do
			if type(k) ~= "number" then
				if type(v) ~= "table" then
					result = result..appendKey(k,v,recurseLevel)
					result = result.." = "
					result = result..appendValue(k,v,recurseLevel)
				else
					table.insert(keysWithTables,k)
				end
			end
		end
		for i = 1, #keysWithTables do
			local k = keysWithTables[i]
			local v = inTable[k]
			if type(k) ~= "number" then
				result = result..appendKey(k,v,recurseLevel)
				result = result.." = "
				result = result..appendValue(k,v,recurseLevel)
			end
		end
		for i = 1, maxIndex do
			local v = inTable[i]
			if v ~= nil then
				result = result..appendKey(i,v,recurseLevel)
				result = result.." = "
				result = result..appendValue(i,v,recurseLevel)
			end
		end

		result = result..indent(recurseLevel-1).."}"
	end

	return result
end



function writetable.createStringFromTable(inTable)

	local result = recursivething(inTable,0)

	return result
end


return writetable