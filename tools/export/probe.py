import unreal

unreal.log_warning("PYPROBE_OK python available")
try:
    im = unreal.InterchangeManager.get_interchange_manager_scripted()
    unreal.log_warning("PYPROBE_INTERCHANGE_OK %s" % type(im))
except Exception as e:
    unreal.log_warning("PYPROBE_INTERCHANGE_FAIL %s" % e)
