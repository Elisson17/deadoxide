//! zombie.scripting.objects.VehicleTemplate: a `template vehicle Name { ... }` block, whose VehicleScript is built
//! the first time a vehicle (or another template) copies from it.
use super::vehicle_script::VehicleScript;

/// zombie.scripting.objects.VehicleTemplate
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleTemplate {
    /// BaseScriptObject.module (its name)
    pub module: String,
    pub name: String,
    /// The whole token, `template vehicle Name { ... }`.
    pub body: String,
    pub script: Option<VehicleScript>,
}

/// What VehicleScript.Load reads from ScriptManager: `getVehicleTemplate` (the index of the template found in
/// `templates`, `None` for null) and the templates, whose scripts are built here on first use.
pub struct TemplateContext<'a> {
    pub get_vehicle_template: &'a dyn Fn(&str) -> Option<usize>,
    pub templates: &'a mut [VehicleTemplate],
    /// The templates whose script is being built, outermost first.
    pub building: Vec<usize>,
    /// The original's DebugLog lines and printed exceptions.
    pub log: Vec<String>,
}

/// Which script `VehicleTemplate.getScript` returned.
pub enum Found {
    /// The template's built script, `templates[t].script`.
    Other,
    /// The script being loaded: a template that copies from itself.
    This,
}

impl VehicleTemplate {
    /// zombie.scripting.objects.VehicleTemplate.<init> with BaseScriptObject.InitLoadPP.
    pub fn new(module: &str, name: &str, body: &str) -> Self {
        VehicleTemplate {
            module: module.to_string(),
            name: name.to_string(),
            body: body.to_string(),
            script: None,
        }
    }

    /// BaseScriptObject.getScriptObjectFullType
    pub fn get_script_object_full_type(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }

    /// zombie.scripting.objects.VehicleTemplate.getScript for `templates[t]`, asked while loading the script of
    /// `this_template` (`None` for a vehicle): on first use a new VehicleScript of the template's module loads the
    /// body (`Load(name, body)`; an exception is printed and the script keeps what loaded before it), then it is
    /// cached. The original stores the script before loading it, so a template that names itself gets the script
    /// being loaded (`Found::This`). A template reached again through another template while it is still being
    /// built would get its partly loaded script; that case is UNKNOWN here (no game template does it): `None`, and
    /// nothing is copied.
    pub fn get_script(
        ctx: &mut TemplateContext,
        t: usize,
        this_template: Option<usize>,
    ) -> Option<Found> {
        if this_template == Some(t) {
            return Some(Found::This);
        }
        if ctx.building.contains(&t) {
            ctx.log.push(format!(
                "UNKNOWN: template \"{}\" reached again while it is being built",
                ctx.templates[t].name
            ));
            return None;
        }
        if ctx.templates[t].script.is_none() {
            let mut script = VehicleScript::new(&ctx.templates[t].module);
            let (name, body) = (ctx.templates[t].name.clone(), ctx.templates[t].body.clone());
            ctx.building.push(t);
            if let Err(e) = script.load(&name, &body, ctx, Some(t), 0) {
                ctx.log.push(e);
            }
            ctx.building.pop();
            ctx.templates[t].script = Some(script);
        }
        Some(Found::Other)
    }
}
