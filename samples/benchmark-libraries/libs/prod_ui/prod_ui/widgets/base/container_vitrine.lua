--[[
A container that holds multiple children, but displays only one of them,
the "presented" widget, at a time. The other children are invisible,
and do not receive update/reshape events.

Pair with a menu (e.g. tabs) to select the presented child.

NOTE: Vitrines do not support the layout system. The active child takes on
the parent's dimensions.
--]]


local context = select(1, ...)


local def = {}


local _mt_widget = context:getLua("core/_mt_widget")


function def:addChild(id, skin_id, pos, ...)
	local wid = _mt_widget.addChild(self, id, skin_id, pos, ...)

	wid.awake = false
	wid.visible = false
	wid.w = 0
	wid.h = 0

	return wid
end


function def:setPresentedChild(wid)
	if wid.parent ~= self then
		error("widget must be a direct child of this parent")
	end

	self.presented_child = wid

	local nodes = self.nodes
	for i, child in ipairs(nodes) do
		if child == wid then
			child.awake = true
			child.visible = true
		else
			child.awake = false
			child.visible = false
			child.w = 0
			child.h = 0
		end
	end

	self:reshape()

	return self
end


function def:getPresentedChild()
	return self.presented_child
end


function def:evt_initialize()
	self.visible = true
	self.allow_hover = true

	self.presented_child = false
end


function def:reshape()
	--print("container_vitrine: reshape (custom)")

	local pres = self.presented_child
	if pres then
		pres.x, pres.y, pres.w, pres.h = 0, 0, self.w, self.h
		if pres.awake then
			pres:reshape()
		end
	end

	self:evt_reshapePost()

	return self
end


--function def:evt_reshapePre()


--function def:evt_reshapePost()


function def:evt_pointerPress(targ, x, y, button, istouch, presses)
	if self == targ then
		-- Try directing thimble1 to the container's UI Frame ancestor.
		if button <= 3 then
			local wid = self
			while wid do
				if wid.frame_type then
					break
				end
				wid = wid.parent
			end
			if wid then
				wid:tryTakeThimble1()
			end
		end
	end
end


return def
