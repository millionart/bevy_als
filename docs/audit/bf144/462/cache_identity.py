import copy
import re

def cache_identity(memory):
 result=copy.deepcopy(memory);versions=set()
 def clean(node):
  if isinstance(node,list):return [clean(x)for x in node]
  if not isinstance(node,dict):return node
  if node.get('name')=='ContainerVersion':
   value=int(node['exported_text'])
   if value!=-1:versions.add(value);node['exported_text']='<hierarchy_instance_hash>'
  for key,value in node.items():
   if key=='exported_text'and isinstance(value,str):node[key]=re.sub(r'ContainerVersion=(-?\d+)',lambda m:'ContainerVersion='+('<hierarchy_instance_hash>'if int(m[1])!=-1 else '-1'),value)
   elif isinstance(value,(list,dict)):node[key]=clean(value)
  return node
 for row in result['properties']:
  if 'FCachedRigElement'in row['cpp_type']:clean(row)
 assert len(versions)==1,versions
 return result
