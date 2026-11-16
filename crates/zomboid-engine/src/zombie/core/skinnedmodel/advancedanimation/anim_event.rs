//! zombie.core.skinnedmodel.advancedanimation.AnimEvent, AnimEventSetVariable, AnimEventFlagWhileAlive
use super::animation_variable_reference::AnimationVariableReference;
use crate::java;
use crate::zombie::util::pz_xml_util::{jaxb_parse_float, XmlElement};

/// zombie.core.skinnedmodel.advancedanimation.AnimEvent.AnimEventTime (`@XmlEnumValue` "Percentage", "Start", "End").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimEventTime {
    Percentage,
    Start,
    End,
}

impl AnimEventTime {
    /// JAXB's `@XmlEnumValue` lookup: the constant whose value is exactly `s`, else null (`None`).
    pub fn from_xml(s: &str) -> Option<AnimEventTime> {
        match s {
            "Percentage" => Some(AnimEventTime::Percentage),
            "Start" => Some(AnimEventTime::Start),
            "End" => Some(AnimEventTime::End),
            _ => None,
        }
    }

    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            AnimEventTime::Percentage => "PERCENTAGE",
            AnimEventTime::Start => "START",
            AnimEventTime::End => "END",
        }
    }
}

/// The subclass `AnimNode.Parse` puts in place of a parsed event.
#[derive(Clone, Debug, PartialEq)]
pub enum AnimEventKind {
    /// zombie.core.skinnedmodel.advancedanimation.AnimEvent itself.
    Plain,
    /// zombie.core.skinnedmodel.advancedanimation.AnimEventSetVariable
    SetVariable {
        variable_reference: AnimationVariableReference,
        set_variable_value: String,
    },
    /// zombie.core.skinnedmodel.advancedanimation.AnimEventFlagWhileAlive
    FlagWhileAlive {
        variable_reference: AnimationVariableReference,
        flag_value: bool,
    },
}

/// zombie.core.skinnedmodel.advancedanimation.AnimEvent: an event fired at a time of the animation.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimEvent {
    pub event_name: Option<String>,
    pub time: Option<AnimEventTime>,
    pub time_pc: f32,
    pub parameter_value: Option<String>,
    pub kind: AnimEventKind,
}

impl Default for AnimEvent {
    /// zombie.core.skinnedmodel.advancedanimation.AnimEvent(): the field initialisers.
    fn default() -> Self {
        AnimEvent {
            event_name: None,
            time: Some(AnimEventTime::Percentage),
            time_pc: 0.0,
            parameter_value: None,
            kind: AnimEventKind::Plain,
        }
    }
}

impl AnimEvent {
    /// JAXB's unmarshalling of `m_EventName`, `m_Time`, `m_TimePc`, `m_ParameterValue`.
    pub fn from_xml(e: &XmlElement) -> Self {
        let mut ev = AnimEvent::default();
        for c in e.elements() {
            match c.name.as_str() {
                "m_EventName" => ev.event_name = Some(c.text()),
                "m_Time" => ev.time = AnimEventTime::from_xml(&c.text()),
                "m_TimePc" => {
                    if let Some(v) = jaxb_parse_float(&c.text()) {
                        ev.time_pc = v;
                    }
                }
                "m_ParameterValue" => ev.parameter_value = Some(c.text()),
                _ => {}
            }
        }
        ev
    }

    /// The parameter split at '=' (`String.split("=")`); `Err` is the NullPointerException of a missing parameter.
    fn split_parameter(&self) -> Result<(Vec<String>, String), String> {
        let p = self.parameter_value.as_deref().ok_or_else(|| {
            "java.lang.NullPointerException: AnimEvent.parameterValue".to_string()
        })?;
        Ok((
            java::split_char(p, '=')
                .into_iter()
                .map(str::to_string)
                .collect(),
            p.to_string(),
        ))
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimEventSetVariable(AnimEvent): `name=value`, else the whole
    /// parameter is the name and the value is "".
    pub fn into_set_variable(self) -> Result<AnimEvent, String> {
        let (ss, p) = self.split_parameter()?;
        let kind = if ss.len() == 2 {
            AnimEventKind::SetVariable {
                variable_reference: AnimationVariableReference::from_raw_variable_name(&ss[0]),
                set_variable_value: ss[1].clone(),
            }
        } else {
            AnimEventKind::SetVariable {
                variable_reference: AnimationVariableReference::from_raw_variable_name(&p),
                set_variable_value: String::new(),
            }
        };
        Ok(AnimEvent { kind, ..self })
    }

    /// zombie.core.skinnedmodel.advancedanimation.AnimEventFlagWhileAlive(AnimEvent): `name=value` with
    /// `Boolean.parseBoolean(value)`, else the whole parameter is the name and the flag is true.
    pub fn into_flag_while_alive(self) -> Result<AnimEvent, String> {
        let (ss, p) = self.split_parameter()?;
        let kind = if ss.len() == 2 {
            AnimEventKind::FlagWhileAlive {
                variable_reference: AnimationVariableReference::from_raw_variable_name(&ss[0]),
                flag_value: java::equals_ignore_case(&ss[1], "true"),
            }
        } else {
            AnimEventKind::FlagWhileAlive {
                variable_reference: AnimationVariableReference::from_raw_variable_name(&p),
                flag_value: true,
            }
        };
        Ok(AnimEvent { kind, ..self })
    }
}
