# 无头探针：确认 pythonscript commandlet 与 unreal 模块可用
import unreal

unreal.log_warning("PYPROBE_OK python available")
# 顺带确认 Interchange glTF 导出器是否注册
try:
    im = unreal.InterchangeManager.get_interchange_manager_scripted()
    unreal.log_warning("PYPROBE_INTERCHANGE_OK %s" % type(im))
except Exception as e:
    unreal.log_warning("PYPROBE_INTERCHANGE_FAIL %s" % e)
