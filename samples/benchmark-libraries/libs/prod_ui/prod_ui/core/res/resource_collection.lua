local resourceCollection = {}


local context = select(1, ...)


local pName = require(context.conf.prod_ui_req .. "lib/p_name")
local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")


local _mt_col = {}
_mt_col.__index = _mt_col


function resourceCollection.newCollection(name)
	uiAssert.typeEval(1, name, "string")

	return setmetatable(pName.set({}, name), _mt_col)
end


function _mt_col:assertObject(id)
	uiAssert.type(1, id, "string")

	local o = self[id]
	if not o then
		error("invalid " .. pName.safeGet(self) .. " ID: " .. id)
	end

	return o
end


-- Reference to an object in a Resource Collection
local _mt_ref = {}
_mt_ref.__index = _mt_ref


function _mt_col:newReference()
	return setmetatable({
		id = false,
		default_id = false,
		collection = self,
		object = false -- ref:refresh()
	}, _mt_ref)
end


-- Call whenever the theme is reloaded.
function _mt_ref:refresh()
	local collection = self.collection
	local object

	if self.id then
		object = collection[self.id]
		if not object then
			error("invalid '" .. pName.safeGet(collection) .. "' ID: " .. tostring(self.id))
		end
	else
		object = collection[self.default_id]
		if not object then
			error("invalid '" .. pName.safeGet(collection) .. "' default ID: " .. tostring(self.default_id))
		end
	end

	self.object = object

	return self
end


function _mt_ref:setId(id)
	uiAssert.typeEval(1, id, "string")

	self.id = id

	self:refresh()

	return self
end


function _mt_ref:setDefaultId(default_id)
	uiAssert.type(1, default_id, "string")

	self.default_id = default_id

	self:refresh()

	return self
end


function _mt_ref:getId()
	return self.id
end


function _mt_ref:getDefaultId()
	return self.default_id
end


function _mt_ref:getActiveId()
	return self.id or self.default_id
end


function _mt_ref:clear()
	self.object = nil
end


function _mt_ref.__call(self)
	if not self.object then
		error("uninitialized object reference")
	end

	return self.object
end


return resourceCollection
