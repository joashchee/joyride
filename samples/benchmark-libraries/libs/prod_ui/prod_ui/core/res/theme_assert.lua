local context = select(1, ...)


local themeAssert = {}


local pName = require(context.conf.prod_ui_req .. "lib.p_name")
local uiAssert = require(context.conf.prod_ui_req .. "ui_assert")
local uiSchema = require(context.conf.prod_ui_req .. "ui_schema")
local uiTable = require(context.conf.prod_ui_req .. "ui_table")
local uiTheme = require(context.conf.prod_ui_req .. "ui_theme")


local function _checkObjectInCollection(col_id, obj_id)
	local collection = context.resources[col_id]
	if not collection then
		error("invalid collection: " .. col_id)

	elseif not collection[obj_id] then
		error("missing object from collection '" .. pName.get(collection) .. "': " .. tostring(v), 2)
	end
end


local function _byPath(col_id, path, eval)
	uiAssert.type(1, col_id, "string")
	if eval then
		uiAssert.type(2, path, "string")
	else
		uiAssert.typeEval(2, path, "string")
	end

	if not eval or (eval and path) then
		local check_col, obj_id = path:match("^%*(.-)/([^/]+)$")
		if not check_col then
			error("failed to split the resource path string: " .. path)

		elseif col_id ~= check_col then
			error("collection ID mismatch: " .. col_id .. " (path: " .. path ..")")
		end

		_checkObjectInCollection(col_id, obj_id)
	end
end


local function _byId(col_id, obj_id, eval)
	uiAssert.type(1, col_id, "string")
	if eval then
		uiAssert.typeEval(2, obj_id, "string")
	else
		uiAssert.type(2, obj_id, "string")
	end

	if not eval or (eval and obj_id) then
		_checkObjectInCollection(col_id, obj_id)
	end
end


themeAssert.path = {}


function themeAssert.path.box(n, v)                _byPath("boxes", v                  ) end
function themeAssert.path.boxEval(n, v)            _byPath("boxes", v,             true) end
function themeAssert.path.buttonStyle(n, v)        _byPath("button_styles", v          ) end
function themeAssert.path.buttonStyleEval(n, v)    _byPath("button_styles", v,     true) end
function themeAssert.path.cardBoxStyle(n, v)       _byPath("card_box_styles", v        ) end
function themeAssert.path.cardBoxStyleEval(n, v)   _byPath("card_box_styles", v,   true) end
function themeAssert.path.font(n, v)               _byPath("fonts", v                  ) end
function themeAssert.path.fontEval(n, v)           _byPath("fonts", v,             true) end
function themeAssert.path.icon(n, v)               _byPath("icons", v                  ) end
function themeAssert.path.iconEval(n, v)           _byPath("icons", v,             true) end
function themeAssert.path.pipeStyle(n, v)          _byPath("pipe_styles", v            ) end
function themeAssert.path.pipeStyleEval(n, v)      _byPath("pipe_styles", v,       true) end
function themeAssert.path.quad(n, v)               _byPath("quads", v                  ) end
function themeAssert.path.quadEval(n, v)           _byPath("quads", v,             true) end
function themeAssert.path.sashStyle(n, v)          _byPath("sash_styles", v            ) end
function themeAssert.path.sashStyleEval(n, v)      _byPath("sash_styles", v,       true) end
function themeAssert.path.scrollBarData(n, v)      _byPath("scroll_bar_data", v        ) end
function themeAssert.path.scrollBarDataEval(n, v)  _byPath("scroll_bar_data", v,   true) end
function themeAssert.path.scrollBarStyle(n, v)     _byPath("scroll_bar_styles", v      ) end
function themeAssert.path.scrollBarStyleEval(n, v) _byPath("scroll_bar_styles", v, true) end
function themeAssert.path.slice(n, v)              _byPath("slices", v                 ) end
function themeAssert.path.sliceEval(n, v)          _byPath("slices", v,            true) end
function themeAssert.path.texture(n, v)            _byPath("textures", v               ) end
function themeAssert.path.textureEval(n, v)        _byPath("textures", v,          true) end


themeAssert.id = {}


function themeAssert.id.box(n, v)                _byId("boxes", v                  ) end
function themeAssert.id.boxEval(n, v)            _byId("boxes", v,             true) end
function themeAssert.id.buttonStyle(n, v)        _byId("button_styles", v          ) end
function themeAssert.id.buttonStyleEval(n, v)    _byId("button_styles", v,     true) end
function themeAssert.id.cardBoxStyle(n, v)       _byId("card_box_styles", v        ) end
function themeAssert.id.cardBoxStyleEval(n, v)   _byId("card_box_styles", v,   true) end
function themeAssert.id.font(n, v)               _byId("fonts", v                  ) end
function themeAssert.id.fontEval(n, v)           _byId("fonts", v,             true) end
function themeAssert.id.icon(n, v)               _byId("icons", v                  ) end
function themeAssert.id.iconEval(n, v)           _byId("icons", v,             true) end
function themeAssert.id.pipeStyle(n, v)          _byId("pipe_styles", v            ) end
function themeAssert.id.pipeStyleEval(n, v)      _byId("pipe_styles", v,       true) end
function themeAssert.id.quad(n, v)               _byId("quads", v                  ) end
function themeAssert.id.quadEval(n, v)           _byId("quads", v,             true) end
function themeAssert.id.sashStyle(n, v)          _byId("sash_styles", v            ) end
function themeAssert.id.sashStyleEval(n, v)      _byId("sash_styles", v,       true) end
function themeAssert.id.scrollBarData(n, v)      _byId("scroll_bar_data", v        ) end
function themeAssert.id.scrollBarDataEval(n, v)  _byId("scroll_bar_data", v,   true) end
function themeAssert.id.scrollBarStyle(n, v)     _byId("scroll_bar_styles", v      ) end
function themeAssert.id.scrollBarStyleEval(n, v) _byId("scroll_bar_styles", v, true) end
function themeAssert.id.slice(n, v)              _byId("slices", v                 ) end
function themeAssert.id.sliceEval(n, v)          _byId("slices", v,            true) end
function themeAssert.id.texture(n, v)            _byId("textures", v               ) end
function themeAssert.id.textureEval(n, v)        _byId("textures", v,          true) end


themeAssert.models = {}


themeAssert.models.default_label_settings = uiSchema.newKeysX {
	font_id = themeAssert.id.font,
	align_x = {uiAssert.namedMap, uiTheme.named_maps.label_align_x},
	align_y = {uiAssert.namedMap, uiTheme.named_maps.label_align_y}
}


return themeAssert
