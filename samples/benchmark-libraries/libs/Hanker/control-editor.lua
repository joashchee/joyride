local Hanker = require 'hanker'
local Controls = Hanker.Controls

local editor = {}

editor.Controls = {}

-- monkey patch
--[[ TODO find a diffo way to do this
function Controls.OnNewControl(control)
	local callInfo = debug.getinfo(3,"Sln")
	local name
	if callInfo.name then
		name = string.format("%s:%d (%s)", callInfo.short_src, callInfo.currentline, callInfo.name)
	else
		name = string.format("%s:%d", callInfo.short_src, callInfo.currentline)
	end
	table.insert(editor.Controls, {
		name, control
	})
end
]]--

function editor:trySelectControl()
	if self.anchorPick then
		--self.applyAnchorPick(self.selectedControl)
	else
		self.selectedControlIndex = self.nextSelectedControlIndex
		self.selectedControlData = self.selectedControls[self.nextSelectedControlIndex]
	end
end

function editor:updateControlLoop()
	self.selectedControls = {}
	local mx, my = love.mouse.getPosition()
	for _, controlData in ipairs(editor.Controls) do
		local control = controlData[2]
		if control:isInside(mx, my) then
			table.insert(self.selectedControls, controlData)
		end
	end

	if #self.selectedControls > 0 then
		if self.selectedControlIndex then
			self.nextSelectedControlIndex = self.selectedControlIndex % #self.selectedControls + 1
		else
			self.nextSelectedControlIndex = 1
		end
	else
		self.selectedControlIndex = nil
		self.nextSelectedControlIndex = nil
	end
end

local function isAltDown()
	return love.keyboard.isDown('lalt', 'ralt')
end

local MOUSE_LEFT = 0
function editor:update()
	if imgui.Begin("Editor") then
		self:updateControlLoop()
		if imgui.IsMouseClicked(MOUSE_LEFT) and isAltDown() then
			self:trySelectControl()
		end

		if self.selectedControlData then
			local controlData = self.selectedControlData
			self:editControl(controlData[1], controlData[2])
		else
			imgui.Text("select a control using alt+lmb")
		end
	end
	imgui.End()

end

local function combo(label, current, opts)
	if imgui.BeginCombo(label, current) then
		for _, opt in ipairs(opts) do
			if imgui.Selectable(opt) then
				current = opt
			end
		end
		imgui.EndCombo()
	end

	return current
end

-- {{{ Do it once, you never have to do it again :p
local function anchorSelectX(anchorX)
	if imgui.Selectable("left", anchorX == "left", 20, 20) then
		return 'left'
	end

	imgui.SameLine()
	if imgui.Selectable("center", anchorX == "center", 20, 20) then
		return 'center'
	end

	imgui.SameLine()
	if imgui.Selectable("right", anchorX == "right", 20, 20) then
		return 'right'
	end
end

local function anchorSelectY(anchorY)
	if imgui.Selectable("top", anchorY == "top") then
		return 'top'
	end

	imgui.SameLine()
	if imgui.Selectable("center", anchorY == "center") then
		return 'center'
	end

	imgui.SameLine()
	if imgui.Selectable("bottom", anchorY == "bottom") then
		return 'bottom'
	end
end

-- WATCH OUT, args reversed for english reasons
local function anchorSelect2D(anchorY, anchorX)
	if imgui.Selectable(
			"top-left",
			anchorY == "top" and anchorX == "left",
			{},
			80, 20) then
		anchorY, anchorX = 'top', 'left'
	end

	imgui.SameLine()
	if imgui.Selectable("top",
			anchorY == "top" and anchorX == "centerX",
			{},
			80, 20) then
		anchorY, anchorX = 'top', 'centerX'
	end

	imgui.SameLine()
	if imgui.Selectable("top-right",
			anchorY == "top" and anchorX == "right",
			{},
			80, 20) then
		anchorY, anchorX = 'top', 'right'
	end

	if imgui.Selectable("left",
			anchorY == "centerY" and anchorX == "left",
			{},
			80, 20) then
		anchorY, anchorX = 'centerY', 'left'
	end

	imgui.SameLine()
	if imgui.Selectable("center",
			anchorY == "centerY" and anchorX == "centerX",
			{},
			80, 20) then
		anchorY, anchorX = 'centerY', 'centerX'
	end

	imgui.SameLine()
	if imgui.Selectable("right",
			anchorY == "centerY" and anchorX == "right",
			{},
			80, 20) then
		anchorY, anchorX = 'centerY', 'right'
	end

	if imgui.Selectable("bottom-left",
			anchorY == "bottom" and anchorX == "left",
			{},
			80, 20) then
		anchorY, anchorX = 'bottom', 'left'
	end

	imgui.SameLine()
	if imgui.Selectable("bottom",
			anchorY == "bottom" and anchorX == "centerX",
			{},
			80, 20) then
		anchorY, anchorX = 'bottom', 'centerX'
	end

	imgui.SameLine()
	if imgui.Selectable("bottom-right",
			anchorY == "bottom" and anchorX == "right",
			{},
			80, 20) then
		anchorY, anchorX = 'bottom', 'right'
	end

	return anchorY, anchorX
end
--}}}

local anchorEditors = {}

function anchorEditors.none(anchor)
end

function anchorEditors.absolute(anchor)
	anchor.value = imgui.DragFloat("Value", anchor.value)
end

local function selectAnchor(name, id, fn)
	imgui.Text(name)
	local anchorName = editor.anchorPick == id and editor.anchorPickName or id.parentAnchor.debugName or "/shrug"
	if imgui.Selectable(anchorName.."##"..tostring(id), editor.anchorPick == id) then
		if editor.anchorPick ~= id then
			editor.anchorPick = id
			editor.applyAnchorPick = fn
		else
			editor.anchorPick = nil
		end
	end
end
function anchorEditors.relative(anchor)
	selectAnchor("Parent Anchor:", anchor, function(newParentAnchor)
		anchor.parentAnchor = newParentAnchor
	end)

	anchor.offset = imgui.DragFloat("Offset", anchor.offset)
end

local function anchorEdit(anchor)
	-- TODO: select type combobox
	--imgui.LabelText("Type", anchor.type)
	if anchor.type ~= 'absolute' then
		imgui.DragFloat("Value", anchor:getValue())
	end

	local editor = anchorEditors[anchor.type] or anchorEditors.none
	editor(anchor)
end

function editor:editControl(name, control)
	imgui.LabelText("Name", name)
	imgui.LabelText("Type", control.ControlType)
	imgui.Separator()

	-- Align
	local alignX, alignY = control.box.x.align, control.box.y.align
	local _alignX = combo("Align X", alignX, {"left", "center", "right"})
	local _alignY = combo("Align Y", alignY, {"top", "center", "bottom"})
	if _alignX ~= alignX or _alignY ~= alignY then
		control:setAlign(_alignX, _alignY)
	end
	local width, height, changed = imgui.DragFloat2("Size", control:getWidth(), control:getHeight(), 1, 0, 1000000)
	if changed then
		control:setWidth(width)
		control:setHeight(height)
	end

	self.targetAnchorY, self.targetAnchorX = anchorSelect2D(self.targetAnchorY, self.targetAnchorX)
	if self.targetAnchorX then
		local anchor = control[self.targetAnchorX](control)
		local nodeName = string.format("%s (%s) ##AnchorX", self.targetAnchorX, anchor.type)
		if imgui.TreeNode(nodeName) then
			anchorEdit(anchor)
			imgui.TreePop()
		end
	end

	if self.targetAnchorY then
		local anchor = control[self.targetAnchorY](control)
		local nodeName = string.format("%s (%s) ##AnchorY", self.targetAnchorY, anchor.type)
		if imgui.TreeNode(nodeName) then
			anchorEdit(anchor)
			imgui.TreePop()
		end
	end
end

return editor
