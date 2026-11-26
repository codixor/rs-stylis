//! CSS selectors.

use crate::compat::Feature;
use crate::error::{ParserError, PrinterError, SelectorError};
use crate::parcel_selectors::parser::{NthSelectorData, NthType, SelectorParseErrorKind};
use crate::parcel_selectors::{
    attr::{AttrSelectorOperator, ParsedAttrSelectorOperation, ParsedCaseSensitivity},
    parser::SelectorImpl,
};
use crate::parser::ParserFlags;
use crate::printer::Printer;
use crate::properties::custom::TokenList;
use crate::rules::StyleContext;
use crate::smallvec_compat::SmallVec;
use crate::stylesheet::{ParserOptions, PrinterOptions};
use crate::targets::{Targets, should_compile};
use crate::traits::{Parse, ParseWithOptions, ToCss};
use crate::values::ident::{CustomIdent, Ident};
use crate::values::string::CSSString;
use crate::vendor_prefix::VendorPrefix;
use crate::{macros::enum_property, values::string::CowArcStr};
use bumpalo::Bump;
use bumpalo::collections::String as BumpString;
use cssparser::*;
use std::collections::HashSet;
use std::fmt;

mod private {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Selectors;
}

use private::Selectors;

/// A list of selectors.
pub type SelectorList<'i> = crate::parcel_selectors::SelectorList<'i, Selectors>;
/// A CSS selector, including a list of components.
pub type Selector<'i> = crate::parcel_selectors::parser::Selector<'i, Selectors>;
/// An individual component within a selector.
pub type Component<'i> = crate::parcel_selectors::parser::Component<'i, Selectors>;
/// A combinator.
pub use crate::parcel_selectors::parser::Combinator;

impl<'i> SelectorImpl<'i> for Selectors {
    type AttrValue = CSSString<'i>;
    type Identifier = Ident<'i>;
    type LocalName = Ident<'i>;
    type NamespacePrefix = Ident<'i>;
    type NamespaceUrl = CowArcStr<'i>;
    type BorrowedNamespaceUrl = CowArcStr<'i>;
    type BorrowedLocalName = Ident<'i>;

    type NonTSPseudoClass = PseudoClass<'i>;
    type PseudoElement = PseudoElement<'i>;
    type VendorPrefix = VendorPrefix;

    type ExtraMatchingData = ();

    fn to_css<W: fmt::Write>(selectors: &SelectorList<'i>, dest: &mut W) -> std::fmt::Result {
        let mut printer = Printer::new(dest, PrinterOptions::default());
        serialize_selector_list(selectors.0.iter(), &mut printer, None, false, None)
            .map_err(|_| std::fmt::Error)
    }
}

pub(crate) struct SelectorParser<'a, 'o, 'i> {
    pub is_nesting_allowed: bool,
    pub options: &'a ParserOptions<'o, 'i>,
}

impl<'a, 'o, 'i> crate::parcel_selectors::parser::Parser<'i> for SelectorParser<'a, 'o, 'i> {
    type Impl = Selectors;
    type Error = ParserError<'i>;

    fn parse_non_ts_pseudo_class(
        &self,
        loc: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<PseudoClass<'i>, ParseError<'i, Self::Error>> {
        use PseudoClass::*;
        let pseudo_class = match_ignore_ascii_case! { &name,
          // https://drafts.csswg.org/selectors-4/#useraction-pseudos
          "hover" => Hover,
          "active" => Active,
          "focus" => Focus,
          "focus-visible" => FocusVisible,
          "focus-within" => FocusWithin,

          // https://drafts.csswg.org/selectors-4/#time-pseudos
          "current" => Current,
          "past" => Past,
          "future" => Future,

          // https://drafts.csswg.org/selectors-4/#resource-pseudos
          "playing" => Playing,
          "paused" => Paused,
          "seeking" => Seeking,
          "buffering" => Buffering,
          "stalled" => Stalled,
          "muted" => Muted,
          "volume-locked" => VolumeLocked,

          // https://fullscreen.spec.whatwg.org/#:fullscreen-pseudo-class
          "fullscreen" => Fullscreen(VendorPrefix::None),
          "-webkit-full-screen" => Fullscreen(VendorPrefix::WebKit),
          "-moz-full-screen" => Fullscreen(VendorPrefix::Moz),
          "-ms-fullscreen" => Fullscreen(VendorPrefix::Ms),

          // https://drafts.csswg.org/selectors/#display-state-pseudos
          "open" => Open,
          "closed" => Closed,
          "modal" => Modal,
          "picture-in-picture" => PictureInPicture,

          // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-popover-open
          "popover-open" => PopoverOpen,

          // https://drafts.csswg.org/selectors-4/#the-defined-pseudo
          "defined" => Defined,

          // https://drafts.csswg.org/selectors-4/#location
          "any-link" => AnyLink(VendorPrefix::None),
          "-webkit-any-link" => AnyLink(VendorPrefix::WebKit),
          "-moz-any-link" => AnyLink(VendorPrefix::Moz),
          "link" => Link,
          "local-link" => LocalLink,
          "target" => Target,
          "target-within" => TargetWithin,
          "visited" => Visited,

          // https://drafts.csswg.org/selectors-4/#input-pseudos
          "enabled" => Enabled,
          "disabled" => Disabled,
          "read-only" => ReadOnly(VendorPrefix::None),
          "-moz-read-only" => ReadOnly(VendorPrefix::Moz),
          "read-write" => ReadWrite(VendorPrefix::None),
          "-moz-read-write" => ReadWrite(VendorPrefix::Moz),
          "placeholder-shown" => PlaceholderShown(VendorPrefix::None),
          "-moz-placeholder" => PlaceholderShown(VendorPrefix::Moz),
          "-ms-input-placeholder" => PlaceholderShown(VendorPrefix::Ms),
          "default" => Default,
          "checked" => Checked,
          "indeterminate" => Indeterminate,
          "blank" => Blank,
          "valid" => Valid,
          "invalid" => Invalid,
          "in-range" => InRange,
          "out-of-range" => OutOfRange,
          "required" => Required,
          "optional" => Optional,
          "user-valid" => UserValid,
          "user-invalid" => UserInvalid,

          // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-autofill
          "autofill" => Autofill(VendorPrefix::None),
          "-webkit-autofill" => Autofill(VendorPrefix::WebKit),
          "-o-autofill" => Autofill(VendorPrefix::O),

          // https://drafts.csswg.org/css-view-transitions-2/#pseudo-classes-for-selective-vt
          "active-view-transition" => ActiveViewTransition,

          // https://webkit.org/blog/363/styling-scrollbars/
          "horizontal" => WebKitScrollbar(WebKitScrollbarPseudoClass::Horizontal),
          "vertical" => WebKitScrollbar(WebKitScrollbarPseudoClass::Vertical),
          "decrement" => WebKitScrollbar(WebKitScrollbarPseudoClass::Decrement),
          "increment" => WebKitScrollbar(WebKitScrollbarPseudoClass::Increment),
          "start" => WebKitScrollbar(WebKitScrollbarPseudoClass::Start),
          "end" => WebKitScrollbar(WebKitScrollbarPseudoClass::End),
          "double-button" => WebKitScrollbar(WebKitScrollbarPseudoClass::DoubleButton),
          "single-button" => WebKitScrollbar(WebKitScrollbarPseudoClass::SingleButton),
          "no-button" => WebKitScrollbar(WebKitScrollbarPseudoClass::NoButton),
          "corner-present" => WebKitScrollbar(WebKitScrollbarPseudoClass::CornerPresent),
          "window-inactive" => WebKitScrollbar(WebKitScrollbarPseudoClass::WindowInactive),

          "local" | "global" if self.options.css_modules.is_some() => {
            return Err(loc.new_custom_error(SelectorParseErrorKind::AmbiguousCssModuleClass(name.clone())))
          },

          _ => {
            if !name.starts_with('-') {
              self.options.warn(loc.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClass(name.clone())));
            }
            Custom { name: name.into() }
          }
        };

        Ok(pseudo_class)
    }

    fn parse_non_ts_functional_pseudo_class<'t>(
        &self,
        name: CowRcStr<'i>,
        parser: &mut cssparser::Parser<'i, 't>,
    ) -> Result<PseudoClass<'i>, ParseError<'i, Self::Error>> {
        use PseudoClass::*;
        let pseudo_class = match_ignore_ascii_case! { &name,
          "lang" => {
            let languages = parser.parse_comma_separated(|parser| {
              parser.expect_ident_or_string()
                .map(|s| s.into())
                .map_err(|e| e.into())
            })?;
            Lang { languages }
          },
          "dir" => Dir { direction: Direction::parse(parser)? },
          // https://drafts.csswg.org/css-view-transitions-2/#the-active-view-transition-type-pseudo
          "active-view-transition-type" => {
            let kind = Parse::parse(parser)?;
            ActiveViewTransitionType { kind }
          },
          "state" => {
            let state = CustomIdent::parse(parser)?;
            State { state }
          },
          "local" if self.options.css_modules.is_some() => Local { selector: Box::new(Selector::parse(self, parser)?) },
          "global" if self.options.css_modules.is_some() => Global { selector: Box::new(Selector::parse(self, parser)?) },
          _ => {
            if !name.starts_with('-') {
              self.options.warn(parser.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClass(name.clone())));
            }
            let mut args = Vec::new();
            TokenList::parse_raw(parser, &mut args, &self.options, 0)?;
            CustomFunction {
              name: name.into(),
              arguments: TokenList(args)
            }
          },
        };

        Ok(pseudo_class)
    }

    fn parse_any_prefix<'t>(&self, name: &str) -> Option<VendorPrefix> {
        match_ignore_ascii_case! { &name,
          "-webkit-any" => Some(VendorPrefix::WebKit),
          "-moz-any" => Some(VendorPrefix::Moz),
          _ => None
        }
    }

    fn parse_pseudo_element(
        &self,
        loc: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<PseudoElement<'i>, ParseError<'i, Self::Error>> {
        use PseudoElement::*;
        let pseudo_element = match_ignore_ascii_case! { &name,
          "before" => Before,
          "after" => After,
          "first-line" => FirstLine,
          "first-letter" => FirstLetter,
          "details-content" => DetailsContent,
          "target-text" => TargetText,
          "cue" => Cue,
          "cue-region" => CueRegion,
          "selection" => Selection(VendorPrefix::None),
          "-moz-selection" => Selection(VendorPrefix::Moz),
          "placeholder" => Placeholder(VendorPrefix::None),
          "-webkit-input-placeholder" => Placeholder(VendorPrefix::WebKit),
          "-moz-placeholder" => Placeholder(VendorPrefix::Moz),
          "-ms-input-placeholder" => Placeholder(VendorPrefix::Ms),
          "marker" => Marker,
          "backdrop" => Backdrop(VendorPrefix::None),
          "-webkit-backdrop" => Backdrop(VendorPrefix::WebKit),
          "file-selector-button" => FileSelectorButton(VendorPrefix::None),
          "-webkit-file-upload-button" => FileSelectorButton(VendorPrefix::WebKit),
          "-ms-browse" => FileSelectorButton(VendorPrefix::Ms),

          "-webkit-scrollbar" => WebKitScrollbar(WebKitScrollbarPseudoElement::Scrollbar),
          "-webkit-scrollbar-button" => WebKitScrollbar(WebKitScrollbarPseudoElement::Button),
          "-webkit-scrollbar-track" => WebKitScrollbar(WebKitScrollbarPseudoElement::Track),
          "-webkit-scrollbar-track-piece" => WebKitScrollbar(WebKitScrollbarPseudoElement::TrackPiece),
          "-webkit-scrollbar-thumb" => WebKitScrollbar(WebKitScrollbarPseudoElement::Thumb),
          "-webkit-scrollbar-corner" => WebKitScrollbar(WebKitScrollbarPseudoElement::Corner),
          "-webkit-resizer" => WebKitScrollbar(WebKitScrollbarPseudoElement::Resizer),

          "picker-icon" => PickerIcon,
          "checkmark" => Checkmark,

          "view-transition" => ViewTransition,

          "grammar-error" => GrammarError,
          "spelling-error" => SpellingError,

          _ => {
            if !name.starts_with('-') {
              self.options.warn(loc.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoElement(name.clone())));
            }
            Custom { name: name.into() }
          }
        };

        Ok(pseudo_element)
    }

    fn parse_functional_pseudo_element<'t>(
        &self,
        name: CowRcStr<'i>,
        arguments: &mut Parser<'i, 't>,
    ) -> Result<<Self::Impl as SelectorImpl<'i>>::PseudoElement, ParseError<'i, Self::Error>> {
        use PseudoElement::*;
        let pseudo_element = match_ignore_ascii_case! { &name,
          "cue" => CueFunction { selector: Box::new(Selector::parse(self, arguments)?) },
          "cue-region" => CueRegionFunction { selector: Box::new(Selector::parse(self, arguments)?) },
          "picker" => PickerFunction { identifier: Ident::parse(arguments)? },
          "view-transition-group" => ViewTransitionGroup { part: ViewTransitionPartSelector::parse(arguments)? },
          "view-transition-image-pair" => ViewTransitionImagePair { part: ViewTransitionPartSelector::parse(arguments)? },
          "view-transition-old" => ViewTransitionOld { part: ViewTransitionPartSelector::parse(arguments)? },
          "view-transition-new" => ViewTransitionNew { part: ViewTransitionPartSelector::parse(arguments)? },
          _ => {
            if !name.starts_with('-') {
              self.options.warn(arguments.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoElement(name.clone())));
            }
            let mut args = Vec::new();
            TokenList::parse_raw(arguments, &mut args, &self.options, 0)?;
            CustomFunction { name: name.into(), arguments: TokenList(args) }
          }
        };

        Ok(pseudo_element)
    }

    #[inline]
    fn parse_slotted(&self) -> bool {
        true
    }

    #[inline]
    fn parse_host(&self) -> bool {
        true
    }

    #[inline]
    fn parse_is_and_where(&self) -> bool {
        true
    }

    #[inline]
    fn parse_part(&self) -> bool {
        true
    }

    fn default_namespace(&self) -> Option<CowArcStr<'i>> {
        None
    }

    fn namespace_for_prefix(&self, prefix: &Ident<'i>) -> Option<CowArcStr<'i>> {
        Some(prefix.0.clone())
    }

    #[inline]
    fn is_nesting_allowed(&self) -> bool {
        self.is_nesting_allowed
    }

    fn deep_combinator_enabled(&self) -> bool {
        self.options
            .flags
            .contains(ParserFlags::DEEP_SELECTOR_COMBINATOR)
    }
}

enum_property! {
  /// The [:dir()](https://drafts.csswg.org/selectors-4/#the-dir-pseudo) pseudo class.
  #[derive(Eq, Hash)]
  pub enum Direction {
    /// Left to right
    Ltr,
    /// Right to left
    Rtl,
  }
}

/// A pseudo class.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum PseudoClass<'i> {
    // https://drafts.csswg.org/selectors-4/#linguistic-pseudos
    /// The [:lang()](https://drafts.csswg.org/selectors-4/#the-lang-pseudo) pseudo class.
    Lang {
        /// A list of language codes.
        languages: Vec<CowArcStr<'i>>,
    },
    /// The [:dir()](https://drafts.csswg.org/selectors-4/#the-dir-pseudo) pseudo class.
    Dir {
        /// A direction.
        direction: Direction,
    },

    // https://drafts.csswg.org/selectors-4/#useraction-pseudos
    /// The [:hover](https://drafts.csswg.org/selectors-4/#the-hover-pseudo) pseudo class.
    Hover,
    /// The [:active](https://drafts.csswg.org/selectors-4/#the-active-pseudo) pseudo class.
    Active,
    /// The [:focus](https://drafts.csswg.org/selectors-4/#the-focus-pseudo) pseudo class.
    Focus,
    /// The [:focus-visible](https://drafts.csswg.org/selectors-4/#the-focus-visible-pseudo) pseudo class.
    FocusVisible,
    /// The [:focus-within](https://drafts.csswg.org/selectors-4/#the-focus-within-pseudo) pseudo class.
    FocusWithin,

    // https://drafts.csswg.org/selectors-4/#time-pseudos
    /// The [:current](https://drafts.csswg.org/selectors-4/#the-current-pseudo) pseudo class.
    Current,
    /// The [:past](https://drafts.csswg.org/selectors-4/#the-past-pseudo) pseudo class.
    Past,
    /// The [:future](https://drafts.csswg.org/selectors-4/#the-future-pseudo) pseudo class.
    Future,

    // https://drafts.csswg.org/selectors-4/#resource-pseudos
    /// The [:playing](https://drafts.csswg.org/selectors-4/#selectordef-playing) pseudo class.
    Playing,
    /// The [:paused](https://drafts.csswg.org/selectors-4/#selectordef-paused) pseudo class.
    Paused,
    /// The [:seeking](https://drafts.csswg.org/selectors-4/#selectordef-seeking) pseudo class.
    Seeking,
    /// The [:buffering](https://drafts.csswg.org/selectors-4/#selectordef-buffering) pseudo class.
    Buffering,
    /// The [:stalled](https://drafts.csswg.org/selectors-4/#selectordef-stalled) pseudo class.
    Stalled,
    /// The [:muted](https://drafts.csswg.org/selectors-4/#selectordef-muted) pseudo class.
    Muted,
    /// The [:volume-locked](https://drafts.csswg.org/selectors-4/#selectordef-volume-locked) pseudo class.
    VolumeLocked,

    /// The [:fullscreen](https://fullscreen.spec.whatwg.org/#:fullscreen-pseudo-class) pseudo class.
    Fullscreen(VendorPrefix),

    // https://drafts.csswg.org/selectors/#display-state-pseudos
    /// The [:open](https://drafts.csswg.org/selectors/#selectordef-open) pseudo class.
    Open,
    /// The [:closed](https://drafts.csswg.org/selectors/#selectordef-closed) pseudo class.
    Closed,
    /// The [:modal](https://drafts.csswg.org/selectors/#modal-state) pseudo class.
    Modal,
    /// The [:picture-in-picture](https://drafts.csswg.org/selectors/#pip-state) pseudo class.
    PictureInPicture,

    // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-popover-open
    /// The [:popover-open](https://html.spec.whatwg.org/multipage/semantics-other.html#selector-popover-open) pseudo class.
    PopoverOpen,

    /// The [:defined](https://drafts.csswg.org/selectors-4/#the-defined-pseudo) pseudo class.
    Defined,

    // https://drafts.csswg.org/selectors-4/#location
    /// The [:any-link](https://drafts.csswg.org/selectors-4/#the-any-link-pseudo) pseudo class.
    AnyLink(VendorPrefix),
    /// The [:link](https://drafts.csswg.org/selectors-4/#link-pseudo) pseudo class.
    Link,
    /// The [:local-link](https://drafts.csswg.org/selectors-4/#the-local-link-pseudo) pseudo class.
    LocalLink,
    /// The [:target](https://drafts.csswg.org/selectors-4/#the-target-pseudo) pseudo class.
    Target,
    /// The [:target-within](https://drafts.csswg.org/selectors-4/#the-target-within-pseudo) pseudo class.
    TargetWithin,
    /// The [:visited](https://drafts.csswg.org/selectors-4/#visited-pseudo) pseudo class.
    Visited,

    // https://drafts.csswg.org/selectors-4/#input-pseudos
    /// The [:enabled](https://drafts.csswg.org/selectors-4/#enabled-pseudo) pseudo class.
    Enabled,
    /// The [:disabled](https://drafts.csswg.org/selectors-4/#disabled-pseudo) pseudo class.
    Disabled,
    /// The [:read-only](https://drafts.csswg.org/selectors-4/#read-only-pseudo) pseudo class.
    ReadOnly(VendorPrefix),
    /// The [:read-write](https://drafts.csswg.org/selectors-4/#read-write-pseudo) pseudo class.
    ReadWrite(VendorPrefix),
    /// The [:placeholder-shown](https://drafts.csswg.org/selectors-4/#placeholder) pseudo class.
    PlaceholderShown(VendorPrefix),
    /// The [:default](https://drafts.csswg.org/selectors-4/#the-default-pseudo) pseudo class.
    Default,
    /// The [:checked](https://drafts.csswg.org/selectors-4/#checked) pseudo class.
    Checked,
    /// The [:indeterminate](https://drafts.csswg.org/selectors-4/#indeterminate) pseudo class.
    Indeterminate,
    /// The [:blank](https://drafts.csswg.org/selectors-4/#blank) pseudo class.
    Blank,
    /// The [:valid](https://drafts.csswg.org/selectors-4/#valid-pseudo) pseudo class.
    Valid,
    /// The [:invalid](https://drafts.csswg.org/selectors-4/#invalid-pseudo) pseudo class.
    Invalid,
    /// The [:in-range](https://drafts.csswg.org/selectors-4/#in-range-pseudo) pseudo class.
    InRange,
    /// The [:out-of-range](https://drafts.csswg.org/selectors-4/#out-of-range-pseudo) pseudo class.
    OutOfRange,
    /// The [:required](https://drafts.csswg.org/selectors-4/#required-pseudo) pseudo class.
    Required,
    /// The [:optional](https://drafts.csswg.org/selectors-4/#optional-pseudo) pseudo class.
    Optional,
    /// The [:user-valid](https://drafts.csswg.org/selectors-4/#user-valid-pseudo) pseudo class.
    UserValid,
    /// The [:used-invalid](https://drafts.csswg.org/selectors-4/#user-invalid-pseudo) pseudo class.
    UserInvalid,

    /// The [:autofill](https://html.spec.whatwg.org/multipage/semantics-other.html#selector-autofill) pseudo class.
    Autofill(VendorPrefix),

    /// The [:active-view-transition](https://drafts.csswg.org/css-view-transitions-2/#the-active-view-transition-pseudo) pseudo class.
    ActiveViewTransition,
    /// The [:active-view-transition-type()](https://drafts.csswg.org/css-view-transitions-2/#the-active-view-transition-type-pseudo) pseudo class.
    ActiveViewTransitionType {
        /// A view transition type.
        kind: SmallVec<CustomIdent<'i>, 1>,
    },

    /// The [:state()](https://developer.mozilla.org/en-US/docs/Web/CSS/:state) pseudo class for custom element states.
    State {
        /// The custom state identifier.
        state: CustomIdent<'i>,
    },

    // CSS modules
    /// The CSS modules :local() pseudo class.
    Local {
        /// A local selector.
        selector: Box<Selector<'i>>,
    },
    /// The CSS modules :global() pseudo class.
    Global {
        /// A global selector.
        selector: Box<Selector<'i>>,
    },

    /// A [webkit scrollbar](https://webkit.org/blog/363/styling-scrollbars/) pseudo class.
    // https://webkit.org/blog/363/styling-scrollbars/
    WebKitScrollbar(WebKitScrollbarPseudoClass),
    /// An unknown pseudo class.
    Custom {
        /// The pseudo class name.
        name: CowArcStr<'i>,
    },
    /// An unknown functional pseudo class.
    CustomFunction {
        /// The pseudo class name.
        name: CowArcStr<'i>,
        /// The arguments of the pseudo class function.
        arguments: TokenList<'i>,
    },
}

/// A [webkit scrollbar](https://webkit.org/blog/363/styling-scrollbars/) pseudo class.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum WebKitScrollbarPseudoClass {
    /// :horizontal
    Horizontal,
    /// :vertical
    Vertical,
    /// :decrement
    Decrement,
    /// :increment
    Increment,
    /// :start
    Start,
    /// :end
    End,
    /// :double-button
    DoubleButton,
    /// :single-button
    SingleButton,
    /// :no-button
    NoButton,
    /// :corner-present
    CornerPresent,
    /// :window-inactive
    WindowInactive,
}

impl<'i> crate::parcel_selectors::parser::NonTSPseudoClass<'i> for PseudoClass<'i> {
    type Impl = Selectors;

    fn is_active_or_hover(&self) -> bool {
        matches!(*self, PseudoClass::Active | PseudoClass::Hover)
    }

    fn is_user_action_state(&self) -> bool {
        matches!(
            *self,
            PseudoClass::Active
                | PseudoClass::Hover
                | PseudoClass::Focus
                | PseudoClass::FocusWithin
                | PseudoClass::FocusVisible
        )
    }

    fn is_valid_before_webkit_scrollbar(&self) -> bool {
        !matches!(*self, PseudoClass::WebKitScrollbar(..))
    }

    fn is_valid_after_webkit_scrollbar(&self) -> bool {
        // https://github.com/WebKit/WebKit/blob/02fbf9b7aa435edb96cbf563a8d4dcf1aa73b4b3/Source/WebCore/css/parser/CSSSelectorParser.cpp#L285
        matches!(
            *self,
            PseudoClass::WebKitScrollbar(..)
                | PseudoClass::Enabled
                | PseudoClass::Disabled
                | PseudoClass::Hover
                | PseudoClass::Active
        )
    }
}

impl<'i> cssparser::ToCss for PseudoClass<'i> {
    fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result
    where
        W: fmt::Write,
    {
        let mut s = String::new();
        serialize_pseudo_class(self, &mut Printer::new(&mut s, Default::default()), None)
            .map_err(|_| std::fmt::Error)?;
        write!(dest, "{}", s)
    }
}

fn serialize_pseudo_class<'a, 'i, W>(
    pseudo_class: &PseudoClass<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    use PseudoClass::*;
    match pseudo_class {
        Lang { languages: lang } => {
            dest.write_str(":lang(")?;
            let mut first = true;
            for lang in lang {
                if first {
                    first = false;
                } else {
                    dest.delim(',', false)?;
                }
                serialize_identifier(lang, dest)?;
            }
            return dest.write_str(")");
        }
        Dir { direction: dir } => {
            dest.write_str(":dir(")?;
            dir.to_css(dest)?;
            return dest.write_str(")");
        }
        State { state } => {
            dest.write_str(":state(")?;
            state.to_css(dest)?;
            return dest.write_str(")");
        }
        _ => {}
    }

    macro_rules! write_prefixed {
        ($prefix: ident, $val: expr) => {{
            dest.write_char(':')?;
            // If the printer has a vendor prefix override, use that.
            let vp = if !dest.vendor_prefix.is_empty() {
                (dest.vendor_prefix & *$prefix).or_none()
            } else {
                *$prefix
            };
            vp.to_css(dest)?;
            dest.write_str($val)
        }};
    }

    macro_rules! pseudo {
        ($key: ident, $s: literal) => {{
            let class = if let Some(pseudo_classes) = &dest.pseudo_classes {
                pseudo_classes.$key
            } else {
                None
            };

            if let Some(class) = class {
                dest.write_char('.')?;
                dest.write_ident(class, true)
            } else {
                dest.write_str($s)
            }
        }};
    }

    match pseudo_class {
        // https://drafts.csswg.org/selectors-4/#useraction-pseudos
        Hover => pseudo!(hover, ":hover"),
        Active => pseudo!(active, ":active"),
        Focus => pseudo!(focus, ":focus"),
        FocusVisible => pseudo!(focus_visible, ":focus-visible"),
        FocusWithin => pseudo!(focus_within, ":focus-within"),

        // https://drafts.csswg.org/selectors-4/#time-pseudos
        Current => dest.write_str(":current"),
        Past => dest.write_str(":past"),
        Future => dest.write_str(":future"),

        // https://drafts.csswg.org/selectors-4/#resource-pseudos
        Playing => dest.write_str(":playing"),
        Paused => dest.write_str(":paused"),
        Seeking => dest.write_str(":seeking"),
        Buffering => dest.write_str(":buffering"),
        Stalled => dest.write_str(":stalled"),
        Muted => dest.write_str(":muted"),
        VolumeLocked => dest.write_str(":volume-locked"),

        // https://fullscreen.spec.whatwg.org/#:fullscreen-pseudo-class
        Fullscreen(prefix) => {
            dest.write_char(':')?;
            let vp = if !dest.vendor_prefix.is_empty() {
                (dest.vendor_prefix & *prefix).or_none()
            } else {
                *prefix
            };
            vp.to_css(dest)?;
            if vp == VendorPrefix::WebKit || vp == VendorPrefix::Moz {
                dest.write_str("full-screen")
            } else {
                dest.write_str("fullscreen")
            }
        }

        // https://drafts.csswg.org/selectors/#display-state-pseudos
        Open => dest.write_str(":open"),
        Closed => dest.write_str(":closed"),
        Modal => dest.write_str(":modal"),
        PictureInPicture => dest.write_str(":picture-in-picture"),

        // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-popover-open
        PopoverOpen => dest.write_str(":popover-open"),

        // https://drafts.csswg.org/selectors-4/#the-defined-pseudo
        Defined => dest.write_str(":defined"),

        // https://drafts.csswg.org/selectors-4/#location
        AnyLink(prefix) => write_prefixed!(prefix, "any-link"),
        Link => dest.write_str(":link"),
        LocalLink => dest.write_str(":local-link"),
        Target => dest.write_str(":target"),
        TargetWithin => dest.write_str(":target-within"),
        Visited => dest.write_str(":visited"),

        // https://drafts.csswg.org/selectors-4/#input-pseudos
        Enabled => dest.write_str(":enabled"),
        Disabled => dest.write_str(":disabled"),
        ReadOnly(prefix) => write_prefixed!(prefix, "read-only"),
        ReadWrite(prefix) => write_prefixed!(prefix, "read-write"),
        PlaceholderShown(prefix) => write_prefixed!(prefix, "placeholder-shown"),
        Default => dest.write_str(":default"),
        Checked => dest.write_str(":checked"),
        Indeterminate => dest.write_str(":indeterminate"),
        Blank => dest.write_str(":blank"),
        Valid => dest.write_str(":valid"),
        Invalid => dest.write_str(":invalid"),
        InRange => dest.write_str(":in-range"),
        OutOfRange => dest.write_str(":out-of-range"),
        Required => dest.write_str(":required"),
        Optional => dest.write_str(":optional"),
        UserValid => dest.write_str(":user-valid"),
        UserInvalid => dest.write_str(":user-invalid"),

        // https://html.spec.whatwg.org/multipage/semantics-other.html#selector-autofill
        Autofill(prefix) => write_prefixed!(prefix, "autofill"),

        ActiveViewTransition => dest.write_str(":active-view-transition"),
        ActiveViewTransitionType { kind } => {
            dest.write_str(":active-view-transition-type(")?;
            kind.to_css(dest)?;
            dest.write_char(')')
        }

        Local { selector } => serialize_selector(selector, dest, context, false, None),
        Global { selector } => {
            let css_module = std::mem::take(&mut dest.css_module);
            serialize_selector(selector, dest, context, false, None)?;
            dest.css_module = css_module;
            Ok(())
        }

        // https://webkit.org/blog/363/styling-scrollbars/
        WebKitScrollbar(s) => {
            use WebKitScrollbarPseudoClass::*;
            dest.write_str(match s {
                Horizontal => ":horizontal",
                Vertical => ":vertical",
                Decrement => ":decrement",
                Increment => ":increment",
                Start => ":start",
                End => ":end",
                DoubleButton => ":double-button",
                SingleButton => ":single-button",
                NoButton => ":no-button",
                CornerPresent => ":corner-present",
                WindowInactive => ":window-inactive",
            })
        }

        Lang { languages: _ } | Dir { direction: _ } | State { .. } => unreachable!(),
        Custom { name } => {
            dest.write_char(':')?;
            return dest.write_str(&name);
        }
        CustomFunction {
            name,
            arguments: args,
        } => {
            dest.write_char(':')?;
            dest.write_str(name)?;
            dest.write_char('(')?;
            args.to_css_raw(dest)?;
            dest.write_char(')')
        }
    }
}

impl<'i> PseudoClass<'i> {
    pub(crate) fn is_equivalent(&self, other: &PseudoClass<'i>) -> bool {
        use PseudoClass::*;
        match (self, other) {
            (Fullscreen(_), Fullscreen(_))
            | (AnyLink(_), AnyLink(_))
            | (ReadOnly(_), ReadOnly(_))
            | (ReadWrite(_), ReadWrite(_))
            | (PlaceholderShown(_), PlaceholderShown(_))
            | (Autofill(_), Autofill(_)) => true,
            (a, b) => a == b,
        }
    }

    pub(crate) fn get_prefix(&self) -> VendorPrefix {
        use PseudoClass::*;
        match self {
            Fullscreen(p) | AnyLink(p) | ReadOnly(p) | ReadWrite(p) | PlaceholderShown(p)
            | Autofill(p) => *p,
            _ => VendorPrefix::empty(),
        }
    }

    pub(crate) fn get_necessary_prefixes(&mut self, targets: Targets) -> VendorPrefix {
        use crate::prefixes::Feature;
        use PseudoClass::*;
        let (p, feature) = match self {
            Fullscreen(p) => (p, Feature::PseudoClassFullscreen),
            AnyLink(p) => (p, Feature::PseudoClassAnyLink),
            ReadOnly(p) => (p, Feature::PseudoClassReadOnly),
            ReadWrite(p) => (p, Feature::PseudoClassReadWrite),
            PlaceholderShown(p) => (p, Feature::PseudoClassPlaceholderShown),
            Autofill(p) => (p, Feature::PseudoClassAutofill),
            _ => return VendorPrefix::empty(),
        };

        *p = targets.prefixes(*p, feature);
        *p
    }
}

/// A pseudo element.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum PseudoElement<'i> {
    /// The [::after](https://drafts.csswg.org/css-pseudo-4/#selectordef-after) pseudo element.
    After,
    /// The [::before](https://drafts.csswg.org/css-pseudo-4/#selectordef-before) pseudo element.
    Before,
    /// The [::first-line](https://drafts.csswg.org/css-pseudo-4/#first-line-pseudo) pseudo element.
    FirstLine,
    /// The [::first-letter](https://drafts.csswg.org/css-pseudo-4/#first-letter-pseudo) pseudo element.
    FirstLetter,
    /// The [::details-content](https://drafts.csswg.org/css-pseudo-4/#details-content-pseudo)
    DetailsContent,
    /// The [::target-text](https://drafts.csswg.org/css-pseudo-4/#selectordef-target-text)
    TargetText,
    /// The [::selection](https://drafts.csswg.org/css-pseudo-4/#selectordef-selection) pseudo element.
    Selection(VendorPrefix),
    /// The [::placeholder](https://drafts.csswg.org/css-pseudo-4/#placeholder-pseudo) pseudo element.
    Placeholder(VendorPrefix),
    ///  The [::marker](https://drafts.csswg.org/css-pseudo-4/#marker-pseudo) pseudo element.
    Marker,
    /// The [::backdrop](https://fullscreen.spec.whatwg.org/#::backdrop-pseudo-element) pseudo element.
    Backdrop(VendorPrefix),
    /// The [::file-selector-button](https://drafts.csswg.org/css-pseudo-4/#file-selector-button-pseudo) pseudo element.
    FileSelectorButton(VendorPrefix),
    /// A [webkit scrollbar](https://webkit.org/blog/363/styling-scrollbars/) pseudo element.
    WebKitScrollbar(WebKitScrollbarPseudoElement),
    /// The [::cue](https://w3c.github.io/webvtt/#the-cue-pseudo-element) pseudo element.
    Cue,
    /// The [::cue-region](https://w3c.github.io/webvtt/#the-cue-region-pseudo-element) pseudo element.
    CueRegion,
    /// The [::cue()](https://w3c.github.io/webvtt/#cue-selector) functional pseudo element.
    CueFunction {
        /// The selector argument.
        selector: Box<Selector<'i>>,
    },
    /// The [::cue-region()](https://w3c.github.io/webvtt/#cue-region-selector) functional pseudo element.
    CueRegionFunction {
        /// The selector argument.
        selector: Box<Selector<'i>>,
    },
    /// The [::view-transition](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#view-transition) pseudo element.
    ViewTransition,
    /// The [::view-transition-group()](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#view-transition-group-pt-name-selector) functional pseudo element.
    ViewTransitionGroup {
        /// A part name selector.
        part: ViewTransitionPartSelector<'i>,
    },
    /// The [::view-transition-image-pair()](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#view-transition-image-pair-pt-name-selector) functional pseudo element.
    ViewTransitionImagePair {
        /// A part name selector.
        part: ViewTransitionPartSelector<'i>,
    },
    /// The [::view-transition-old()](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#view-transition-old-pt-name-selector) functional pseudo element.
    ViewTransitionOld {
        /// A part name selector.
        part: ViewTransitionPartSelector<'i>,
    },
    /// The [::view-transition-new()](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#view-transition-new-pt-name-selector) functional pseudo element.
    ViewTransitionNew {
        /// A part name selector.
        part: ViewTransitionPartSelector<'i>,
    },
    /// The [::picker()](https://drafts.csswg.org/css-forms-1/#the-picker-pseudo-element) functional pseudo element.
    PickerFunction {
        /// A form control identifier.
        identifier: Ident<'i>,
    },
    /// The [::picker-icon](https://drafts.csswg.org/css-forms-1/#picker-opener-icon-the-picker-icon-pseudo-element) pseudo element.
    PickerIcon,
    /// The [::checkmark](https://drafts.csswg.org/css-forms-1/#styling-checkmarks-the-checkmark-pseudo-element) pseudo element.
    Checkmark,
    /// The [::grammar-error](https://drafts.csswg.org/css-pseudo/#selectordef-grammar-error) pseudo element.
    GrammarError,
    /// The [::spelling-error](https://drafts.csswg.org/css-pseudo/#selectordef-spelling-error) pseudo element.
    SpellingError,
    /// An unknown pseudo element.
    Custom {
        /// The name of the pseudo element.
        name: CowArcStr<'i>,
    },
    /// An unknown functional pseudo element.
    CustomFunction {
        ///The name of the pseudo element.
        name: CowArcStr<'i>,
        /// The arguments of the pseudo element function.
        arguments: TokenList<'i>,
    },
}

/// A [webkit scrollbar](https://webkit.org/blog/363/styling-scrollbars/) pseudo element.
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub enum WebKitScrollbarPseudoElement {
    /// ::-webkit-scrollbar
    Scrollbar,
    /// ::-webkit-scrollbar-button
    Button,
    /// ::-webkit-scrollbar-track
    Track,
    /// ::-webkit-scrollbar-track-piece
    TrackPiece,
    /// ::-webkit-scrollbar-thumb
    Thumb,
    /// ::-webkit-scrollbar-corner
    Corner,
    /// ::-webkit-resizer
    Resizer,
}

/// A [view transition part name](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#typedef-pt-name-selector).
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub enum ViewTransitionPartName<'i> {
    /// *
    All,
    /// <custom-ident>
    Name(CustomIdent<'i>),
}

impl<'i> Parse<'i> for ViewTransitionPartName<'i> {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        if input.try_parse(|input| input.expect_delim('*')).is_ok() {
            return Ok(ViewTransitionPartName::All);
        }

        Ok(ViewTransitionPartName::Name(CustomIdent::parse(input)?))
    }
}

impl<'i> ToCss for ViewTransitionPartName<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        match self {
            ViewTransitionPartName::All => dest.write_char('*'),
            ViewTransitionPartName::Name(name) => name.to_css(dest),
        }
    }
}

/// A [view transition part selector](https://w3c.github.io/csswg-drafts/css-view-transitions-1/#typedef-pt-name-selector).
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub struct ViewTransitionPartSelector<'i> {
    /// The view transition part name.
    name: Option<ViewTransitionPartName<'i>>,
    /// A list of view transition classes.
    classes: Vec<CustomIdent<'i>>,
}

impl<'i> Parse<'i> for ViewTransitionPartSelector<'i> {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        input.skip_whitespace();
        let name = input.try_parse(ViewTransitionPartName::parse).ok();
        let mut classes = Vec::new();
        while let Ok(token) = input.next_including_whitespace() {
            if matches!(token, Token::Delim('.')) {
                match input.next_including_whitespace() {
                    Ok(Token::Ident(id)) => classes.push(CustomIdent(id.into())),
                    _ => {
                        return Err(input.new_custom_error(ParserError::SelectorError(
                            SelectorError::InvalidState,
                        )));
                    }
                }
            } else {
                break;
            }
        }

        if !input.is_exhausted() || (name.is_none() && classes.is_empty()) {
            return Err(
                input.new_custom_error(ParserError::SelectorError(SelectorError::InvalidState))
            );
        }

        Ok(ViewTransitionPartSelector { name, classes })
    }
}

impl<'i> ToCss for ViewTransitionPartSelector<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: std::fmt::Write,
    {
        if let Some(name) = &self.name {
            name.to_css(dest)?;
        }
        for class in &self.classes {
            dest.write_char('.')?;
            class.to_css(dest)?;
        }
        Ok(())
    }
}

impl<'i> cssparser::ToCss for PseudoElement<'i> {
    fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result
    where
        W: fmt::Write,
    {
        let mut s = String::new();
        serialize_pseudo_element(
            self,
            &mut Printer::new(&mut s, Default::default()),
            None,
            None,
        )
        .map_err(|_| std::fmt::Error)?;
        write!(dest, "{}", s)
    }
}

fn serialize_pseudo_element<'a, 'i, W>(
    pseudo_element: &PseudoElement<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
    selector_source: Option<&str>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    use PseudoElement::*;

    macro_rules! write_prefix {
        ($prefix: ident) => {{
            dest.write_str("::")?;
            // If the printer has a vendor prefix override, use that.
            let vp = if !dest.vendor_prefix.is_empty() {
                (dest.vendor_prefix & *$prefix).or_none()
            } else {
                *$prefix
            };
            vp.to_css(dest)?;
            vp
        }};
    }

    macro_rules! write_prefixed {
        ($prefix: ident, $val: expr) => {{
            write_prefix!($prefix);
            dest.write_str($val)
        }};
    }

    let react_css2_pseudo = |legacy: &str, modern: &str, dest: &mut Printer<W>| {
        if dest.is_react_compat() {
            match selector_source.map(str::trim) {
                Some(source) if source.contains(modern) => dest.write_str(modern),
                Some(source) if source.contains(legacy) => dest.write_str(legacy),
                _ => dest.write_str(modern),
            }
        } else {
            dest.write_str(legacy)
        }
    };

    match pseudo_element {
        // CSS2 pseudo elements support a single colon syntax in addition
        // to the more correct double colon for other pseudo elements.
        // JS stylis preserves the authored single- vs double-colon form in React mode.
        After => react_css2_pseudo(":after", "::after", dest),
        Before => react_css2_pseudo(":before", "::before", dest),
        FirstLine => react_css2_pseudo(":first-line", "::first-line", dest),
        FirstLetter => react_css2_pseudo(":first-letter", "::first-letter", dest),
        DetailsContent => dest.write_str("::details-content"),
        TargetText => dest.write_str("::target-text"),
        Marker => dest.write_str("::marker"),
        Selection(prefix) => write_prefixed!(prefix, "selection"),
        Cue => dest.write_str("::cue"),
        CueRegion => dest.write_str("::cue-region"),
        CueFunction { selector } => {
            dest.write_str("::cue(")?;
            serialize_selector(selector, dest, context, false, None)?;
            dest.write_char(')')
        }
        CueRegionFunction { selector } => {
            dest.write_str("::cue-region(")?;
            serialize_selector(selector, dest, context, false, None)?;
            dest.write_char(')')
        }
        Placeholder(prefix) => {
            let vp = if !dest.vendor_prefix.is_empty() {
                (dest.vendor_prefix & *prefix).or_none()
            } else {
                *prefix
            };
            if vp == VendorPrefix::Ms {
                dest.write_str(":-ms-input-placeholder")
            } else {
                dest.write_str("::")?;
                vp.to_css(dest)?;
                if vp == VendorPrefix::WebKit {
                    dest.write_str("input-placeholder")
                } else {
                    dest.write_str("placeholder")
                }
            }
        }
        Backdrop(prefix) => write_prefixed!(prefix, "backdrop"),
        FileSelectorButton(prefix) => {
            let vp = write_prefix!(prefix);
            if vp == VendorPrefix::WebKit {
                dest.write_str("file-upload-button")
            } else if vp == VendorPrefix::Ms {
                dest.write_str("browse")
            } else {
                dest.write_str("file-selector-button")
            }
        }
        WebKitScrollbar(s) => {
            use WebKitScrollbarPseudoElement::*;
            dest.write_str(match s {
                Scrollbar => "::-webkit-scrollbar",
                Button => "::-webkit-scrollbar-button",
                Track => "::-webkit-scrollbar-track",
                TrackPiece => "::-webkit-scrollbar-track-piece",
                Thumb => "::-webkit-scrollbar-thumb",
                Corner => "::-webkit-scrollbar-corner",
                Resizer => "::-webkit-resizer",
            })
        }
        ViewTransition => dest.write_str("::view-transition"),
        ViewTransitionGroup { part } => {
            dest.write_str("::view-transition-group(")?;
            part.to_css(dest)?;
            dest.write_char(')')
        }
        ViewTransitionImagePair { part } => {
            dest.write_str("::view-transition-image-pair(")?;
            part.to_css(dest)?;
            dest.write_char(')')
        }
        ViewTransitionOld { part } => {
            dest.write_str("::view-transition-old(")?;
            part.to_css(dest)?;
            dest.write_char(')')
        }
        ViewTransitionNew { part } => {
            dest.write_str("::view-transition-new(")?;
            part.to_css(dest)?;
            dest.write_char(')')
        }
        PickerFunction { identifier } => {
            dest.write_str("::picker(")?;
            identifier.to_css(dest)?;
            dest.write_char(')')
        }
        PickerIcon => dest.write_str("::picker-icon"),
        Checkmark => dest.write_str("::checkmark"),
        GrammarError => dest.write_str("::grammar-error"),
        SpellingError => dest.write_str("::spelling-error"),
        Custom { name: val } => {
            dest.write_str("::")?;
            return dest.write_str(val);
        }
        CustomFunction {
            name,
            arguments: args,
        } => {
            dest.write_str("::")?;
            dest.write_str(name)?;
            dest.write_char('(')?;
            args.to_css_raw(dest)?;
            dest.write_char(')')
        }
    }
}

impl<'i> crate::parcel_selectors::parser::PseudoElement<'i> for PseudoElement<'i> {
    type Impl = Selectors;

    fn accepts_state_pseudo_classes(&self) -> bool {
        // Be lenient.
        true
    }

    fn valid_after_slotted(&self) -> bool {
        // ::slotted() should support all tree-abiding pseudo-elements, see
        // https://drafts.csswg.org/css-scoping/#slotted-pseudo
        // https://drafts.csswg.org/css-pseudo-4/#treelike
        matches!(
            *self,
            PseudoElement::Before
                | PseudoElement::After
                | PseudoElement::Marker
                | PseudoElement::Placeholder(_)
                | PseudoElement::FileSelectorButton(_)
        )
    }

    fn is_webkit_scrollbar(&self) -> bool {
        matches!(*self, PseudoElement::WebKitScrollbar(..))
    }

    fn is_view_transition(&self) -> bool {
        matches!(
            *self,
            PseudoElement::ViewTransitionGroup { .. }
                | PseudoElement::ViewTransitionImagePair { .. }
                | PseudoElement::ViewTransitionNew { .. }
                | PseudoElement::ViewTransitionOld { .. }
        )
    }

    fn is_unknown(&self) -> bool {
        matches!(
            *self,
            PseudoElement::Custom { .. } | PseudoElement::CustomFunction { .. },
        )
    }
}

impl<'i> PseudoElement<'i> {
    pub(crate) fn is_equivalent(&self, other: &PseudoElement<'i>) -> bool {
        use PseudoElement::*;
        match (self, other) {
            (Selection(_), Selection(_))
            | (Placeholder(_), Placeholder(_))
            | (Backdrop(_), Backdrop(_))
            | (FileSelectorButton(_), FileSelectorButton(_)) => true,
            (a, b) => a == b,
        }
    }

    pub(crate) fn get_prefix(&self) -> VendorPrefix {
        use PseudoElement::*;
        match self {
            Selection(p) | Placeholder(p) | Backdrop(p) | FileSelectorButton(p) => *p,
            _ => VendorPrefix::empty(),
        }
    }

    pub(crate) fn get_necessary_prefixes(&mut self, targets: Targets) -> VendorPrefix {
        use crate::prefixes::Feature;
        use PseudoElement::*;
        let (p, feature) = match self {
            Selection(p) => (p, Feature::PseudoElementSelection),
            Placeholder(p) => (p, Feature::PseudoElementPlaceholder),
            Backdrop(p) => (p, Feature::PseudoElementBackdrop),
            FileSelectorButton(p) => (p, Feature::PseudoElementFileSelectorButton),
            _ => return VendorPrefix::empty(),
        };

        *p = targets.prefixes(*p, feature);
        *p
    }
}

impl<'a, 'i> ToCss for SelectorList<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: fmt::Write,
    {
        to_css_with_source(self, dest, None, None)
    }
}

pub(crate) fn to_css_with_source<'i, W>(
    selectors: &SelectorList<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
    selector_source: Option<&str>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    serialize_selector_list(selectors.0.iter(), dest, context, false, selector_source)
}

pub(crate) fn to_css_with_namespace_middleware<'i, W>(
    selectors: &SelectorList<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    let mut first = true;
    for selector in &selectors.0 {
        if !first {
            dest.delim(',', false)?;
        }
        first = false;
        let selector_text = serialize_selector_for_namespace(selector, dest, context)?;
        let namespaced = namespace_selector(&selector_text);
        dest.write_str(&namespaced)?;
    }

    Ok(())
}

impl ToCss for Combinator {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: fmt::Write,
    {
        match *self {
            Combinator::Child => dest.delim('>', true),
            Combinator::Descendant => dest.write_str(" "),
            Combinator::NextSibling => dest.delim('+', true),
            Combinator::LaterSibling => dest.delim('~', true),
            Combinator::Deep => dest.write_str(" /deep/ "),
            Combinator::DeepDescendant => {
                dest.whitespace()?;
                dest.write_str(">>>")?;
                dest.whitespace()
            }
            Combinator::PseudoElement | Combinator::Part | Combinator::SlotAssignment => Ok(()),
        }
    }
}

fn serialize_react_relative_combinator<W>(
    combinator: Combinator,
    dest: &mut Printer<W>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    match combinator {
        Combinator::Child => {
            dest.write_char('>')?;
            dest.write_char(' ')
        }
        Combinator::NextSibling => {
            dest.write_char('+')?;
            dest.write_char(' ')
        }
        Combinator::LaterSibling => {
            dest.write_char('~')?;
            dest.write_char(' ')
        }
        _ => combinator.to_css(dest),
    }
}

fn serialize_react_nesting_join_combinator<W>(
    combinator: Combinator,
    selector_source: Option<&str>,
    dest: &mut Printer<W>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    let uses_implicit_relative = selector_source.map(str::trim_start).is_some_and(|source| {
        matches!(
            source.as_bytes().first(),
            Some(b'>') | Some(b'+') | Some(b'~')
        )
    });

    match combinator {
        Combinator::Child if uses_implicit_relative => {
            dest.write_char(' ')?;
            dest.write_char('>')
        }
        Combinator::NextSibling if uses_implicit_relative => {
            dest.write_char(' ')?;
            dest.write_char('+')
        }
        Combinator::LaterSibling if uses_implicit_relative => {
            dest.write_char(' ')?;
            dest.write_char('~')
        }
        _ => combinator.to_css(dest),
    }
}

// Copied from the selectors crate and modified to override to_css implementation.
impl<'a, 'i> ToCss for Selector<'i> {
    fn to_css<W>(&self, dest: &mut Printer<W>) -> Result<(), PrinterError>
    where
        W: fmt::Write,
    {
        serialize_selector(self, dest, None, false, None)
    }
}

fn serialize_selector<'a, 'i, W>(
    selector: &Selector<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
    mut is_relative: bool,
    selector_source: Option<&str>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    serialize_selector_inner(
        selector,
        dest,
        context,
        &mut is_relative,
        None,
        selector_source,
    )
}

fn serialize_selector_inner<'a, 'i, W>(
    selector: &Selector<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
    is_relative: &mut bool,
    leading_nesting_override: Option<&str>,
    selector_source: Option<&str>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    use crate::parcel_selectors::parser::*;
    // Compound selectors invert the order of their contents, so we need to
    // undo that during serialization.
    //
    // This two-iterator strategy involves walking over the selector twice.
    // We could do something more clever, but selector serialization probably
    // isn't hot enough to justify it, and the stringification likely
    // dominates anyway.
    //
    // NB: A parse-order iterator is a Rev<>, which doesn't expose as_slice(),
    // which we need for |split|. So we split by combinators on a match-order
    // sequence and then reverse.

    let combinators = selector
        .iter_raw_match_order()
        .rev()
        .filter_map(|x| x.as_combinator())
        .collect::<SmallVec<_, 8>>();
    let compound_selectors = selector
        .iter_raw_match_order()
        .as_slice()
        .split(|x| x.is_combinator())
        .rev()
        .map(|compound| compound.iter().cloned().collect::<SmallVec<_, 8>>())
        .collect::<SmallVec<_, 8>>();
    let should_compile_nesting = should_compile!(dest.targets.current, Nesting);
    let has_distributable_leading_nesting =
        selector_has_distributable_leading_nesting(&compound_selectors, &combinators);
    let mut first_branch = true;

    if leading_nesting_override.is_none()
        && dest.minify
        && dest.is_react_compat()
        && should_compile_nesting
        && has_distributable_leading_nesting
        && with_expanded_context_branches(
            dest.targets.current,
            dest.normalization_profile,
            dest.vendor_prefix,
            context,
            |branch| {
                if !first_branch {
                    dest.delim(',', false)?;
                }
                first_branch = false;
                let mut branch_is_relative = *is_relative;
                serialize_selector_inner(
                    selector,
                    dest,
                    context,
                    &mut branch_is_relative,
                    Some(branch),
                    selector_source,
                )
            },
        )?
    {
        return Ok(());
    }

    if leading_nesting_override.is_none()
        && dest.minify
        && dest.is_react_compat()
        && should_compile_nesting
        && let Some(ctx) = context
        && ctx.selectors.0.len() > 1
        && ctx.selectors.0.iter().any(has_nesting)
        && let Some(parent_ctx) = ctx.parent
        && parent_ctx.selectors.0.len() > 1
    {
        let current_selectors = crate::parcel_selectors::SelectorList(ctx.selectors.0.clone());
        let grandparent = parent_ctx.parent;
        let mut first_parent = true;

        for parent_selector in &parent_ctx.selectors.0 {
            if !first_parent {
                dest.delim(',', false)?;
            }
            first_parent = false;

            let parent_selectors = SelectorList::from(parent_selector.clone());
            let lifted_parent_context = StyleContext {
                selectors: &parent_selectors,
                selector_source: parent_ctx.selector_source,
                parent: grandparent,
            };
            let lifted_context = StyleContext {
                selectors: &current_selectors,
                selector_source: ctx.selector_source,
                parent: Some(&lifted_parent_context),
            };
            let mut branch_is_relative = *is_relative;
            serialize_selector_inner(
                selector,
                dest,
                Some(&lifted_context),
                &mut branch_is_relative,
                None,
                selector_source,
            )?;
        }
        return Ok(());
    }

    if leading_nesting_override.is_none()
        && dest.minify
        && dest.is_react_compat()
        && should_compile_nesting
        && let Some(ctx) = context
        && ctx.selectors.0.len() > 1
    {
        let parent = ctx.parent;
        let mut first_parent = true;
        for parent_selector in &ctx.selectors.0 {
            if !first_parent {
                dest.delim(',', false)?;
            }
            first_parent = false;

            let parent_selectors = SelectorList::from(parent_selector.clone());
            let parent_context = StyleContext {
                selectors: &parent_selectors,
                selector_source: ctx.selector_source,
                parent,
            };
            let mut branch_is_relative = *is_relative;
            serialize_selector_inner(
                selector,
                dest,
                Some(&parent_context),
                &mut branch_is_relative,
                None,
                selector_source,
            )?;
        }
        return Ok(());
    }

    if leading_nesting_override.is_none()
        && dest.minify
        && dest.is_react_compat()
        && should_compile_nesting
        && let Some(ctx) = context
        && ctx.selectors.0.len() == 1
        && has_nesting(&ctx.selectors.0[0])
        && let Some(parent_ctx) = ctx.parent
        && parent_ctx.selectors.0.len() > 1
    {
        let current_selector = ctx.selectors.0[0].clone();
        let grandparent = parent_ctx.parent;
        let current_selector_list = SelectorList::from(current_selector);
        let mut first_parent = true;

        for parent_selector in &parent_ctx.selectors.0 {
            if !first_parent {
                dest.delim(',', false)?;
            }
            first_parent = false;

            let parent_selectors = SelectorList::from(parent_selector.clone());
            let lifted_parent_context = StyleContext {
                selectors: &parent_selectors,
                selector_source: parent_ctx.selector_source,
                parent: grandparent,
            };
            let lifted_context = StyleContext {
                selectors: &current_selector_list,
                selector_source: ctx.selector_source,
                parent: Some(&lifted_parent_context),
            };
            let mut branch_is_relative = *is_relative;
            serialize_selector_inner(
                selector,
                dest,
                Some(&lifted_context),
                &mut branch_is_relative,
                None,
                selector_source,
            )?;
        }
        return Ok(());
    }

    let mut leading_nesting_override = leading_nesting_override;
    let component_context = if leading_nesting_override.is_some() {
        None
    } else {
        context
    };
    let mut first = true;
    let mut combinators_exhausted = false;
    let mut combinator_index = 0usize;
    for mut compound in compound_selectors
        .iter()
        .map(|compound| compound.as_slice())
    {
        debug_assert!(!combinators_exhausted);
        let mut used_leading_nesting = false;

        // Skip implicit :scope in relative selectors (e.g. :has(:scope > foo) -> :has(> foo))
        if *is_relative && matches!(compound.get(0), Some(Component::Scope)) {
            if let Some(combinator) = combinators.get(combinator_index).copied() {
                combinator_index += 1;
                if dest.minify && dest.is_react_compat() {
                    serialize_react_relative_combinator(combinator, dest)?;
                } else {
                    combinator.to_css(dest)?;
                }
            }
            compound = &compound[1..];
            *is_relative = false;
        }

        // https://drafts.csswg.org/cssom/#serializing-selectors
        if compound.is_empty() {
            continue;
        }

        let has_leading_nesting = first && matches!(compound[0], Component::Nesting);
        let first_index = if has_leading_nesting { 1 } else { 0 };
        first = false;

        // 1. If there is only one simple selector in the compound selectors
        //    which is a universal selector, append the result of
        //    serializing the universal selector to s.
        //
        // Check if `!compound.empty()` first--this can happen if we have
        // something like `... > ::before`, because we store `>` and `::`
        // both as combinators internally.
        //
        // If we are in this case, after we have serialized the universal
        // selector, we skip Step 2 and continue with the algorithm.
        let (can_elide_namespace, first_non_namespace) = match compound.get(first_index) {
            Some(Component::ExplicitAnyNamespace)
            | Some(Component::ExplicitNoNamespace)
            | Some(Component::Namespace(..)) => (false, first_index + 1),
            Some(Component::DefaultNamespace(..)) => (true, first_index + 1),
            _ => (true, first_index),
        };
        let mut perform_step_2 = true;
        let next_combinator = combinators.get(combinator_index).copied();
        if first_non_namespace == compound.len() - 1 {
            match (next_combinator, &compound[first_non_namespace]) {
                // We have to be careful here, because if there is a
                // pseudo element "combinator" there isn't really just
                // the one simple selector. Technically this compound
                // selector contains the pseudo element selector as well
                // -- Combinator::PseudoElement, just like
                // Combinator::SlotAssignment, don't exist in the
                // spec.
                (Some(Combinator::PseudoElement), _) | (Some(Combinator::SlotAssignment), _) => (),
                (_, &Component::ExplicitUniversalType) => {
                    // Iterate over everything so we serialize the namespace
                    // too.
                    let mut iter = compound.iter();
                    let swap_nesting = has_leading_nesting && should_compile_nesting;
                    if swap_nesting {
                        // Swap nesting and type selector (e.g. &div -> div&).
                        iter.next();
                    }

                    for simple in iter {
                        serialize_component(simple, dest, context, selector_source)?;
                    }

                    if swap_nesting {
                        serialize_nesting(dest, context, false)?;
                    }

                    // Skip step 2, which is an "otherwise".
                    perform_step_2 = false;
                }
                _ => (),
            }
        }

        // 2. Otherwise, for each simple selector in the compound selectors
        //    that is not a universal selector of which the namespace prefix
        //    maps to a namespace that is not the default namespace
        //    serialize the simple selector and append the result to s.
        //
        // See https://github.com/w3c/csswg-drafts/issues/1606, which is
        // proposing to change this to match up with the behavior asserted
        // in cssom/serialize-namespaced-type-selectors.html, which the
        // following code tries to match.
        if perform_step_2 {
            let mut iter = compound.iter();
            if has_leading_nesting
                && should_compile_nesting
                && is_type_selector(compound.get(first_non_namespace))
                && !is_suffix_nesting_type_selector(compound.get(first_non_namespace))
            {
                used_leading_nesting = true;
                // Swap nesting and type selector (e.g. &div -> div&).
                // This ensures that the compiled selector is valid. e.g. (div.foo is valid, .foodiv is not).
                let nesting = iter.next().unwrap();
                let local = iter.next().unwrap();
                serialize_component(local, dest, component_context, selector_source)?;

                // Also check the next item in case of namespaces.
                if first_non_namespace > first_index {
                    let local = iter.next().unwrap();
                    serialize_component(local, dest, component_context, selector_source)?;
                }

                serialize_component(nesting, dest, component_context, selector_source)?;
            } else if has_leading_nesting && should_compile_nesting {
                used_leading_nesting = true;
                // Nesting selector may serialize differently if it is leading, due to type selectors.
                iter.next();
                if let Some(override_selector) = leading_nesting_override.take() {
                    dest.write_str(override_selector)?;
                } else {
                    serialize_nesting(dest, context, true)?;
                }
            }

            for simple in iter {
                if let Component::ExplicitUniversalType = *simple {
                    // Can't have a namespace followed by a pseudo-element
                    // selector followed by a universal selector in the same
                    // compound selector, so we don't have to worry about the
                    // real namespace being in a different `compound`.
                    if can_elide_namespace && !(dest.is_react_compat() && compound.len() > 1) {
                        continue;
                    }
                }
                serialize_component(simple, dest, component_context, selector_source)?;
            }
        }

        // 3. If this is not the last part of the chain of the selector
        //    append a single SPACE (U+0020), followed by the combinator
        //    ">", "+", "~", ">>", "||", as appropriate, followed by another
        //    single SPACE (U+0020) if the combinator was not whitespace, to
        //    s.
        match next_combinator {
            Some(c) => {
                if dest.minify && dest.is_react_compat() && used_leading_nesting {
                    serialize_react_nesting_join_combinator(c, selector_source, dest)?;
                } else {
                    c.to_css(dest)?;
                }
                combinator_index += 1;
            }
            None => combinators_exhausted = true,
        };

        // 4. If this is the last part of the chain of the selector and
        //    there is a pseudo-element, append "::" followed by the name of
        //    the pseudo-element, to s.
        //
        // (we handle this above)
    }

    Ok(())
}

fn selector_has_distributable_leading_nesting(
    compound_selectors: &[SmallVec<Component<'_>, 8>],
    combinators: &[Combinator],
) -> bool {
    let Some(first_compound) = compound_selectors.first() else {
        return false;
    };

    matches!(first_compound.first(), Some(Component::Nesting))
        && (first_compound.len() > 1 || !combinators.is_empty() || compound_selectors.len() > 1)
}

fn with_expanded_context_branches<F>(
    targets: Targets,
    normalization_profile: crate::react_compat::NormalizationProfile,
    vendor_prefix: VendorPrefix,
    context: Option<&StyleContext<'_, '_>>,
    mut f: F,
) -> Result<bool, PrinterError>
where
    F: FnMut(&str) -> Result<(), PrinterError>,
{
    let Some(context) = context else {
        return Ok(false);
    };

    let bump = Bump::new();
    let mut serialized = BumpString::new_in(&bump);
    {
        let mut printer = Printer::new(
            &mut serialized,
            PrinterOptions {
                minify: true,
                targets,
                normalization_profile,
                ..PrinterOptions::default()
            },
        );
        printer.vendor_prefix = vendor_prefix;
        to_css_with_source(
            context.selectors,
            &mut printer,
            context.parent,
            context.selector_source,
        )?;
    }

    let branches = split_top_level_selector_list(&serialized);
    if branches.len() > 1 {
        for branch in branches {
            f(branch)?;
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

fn split_top_level_selector_list<'a>(serialized: &'a str) -> SmallVec<&'a str, 8> {
    let bytes = serialized.as_bytes();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut selectors = SmallVec::new();

    while index < bytes.len() {
        let byte = bytes[index];

        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }

        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'(' => paren_depth += 1,
            b')' if paren_depth > 0 => paren_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' if bracket_depth > 0 => bracket_depth -= 1,
            b',' if paren_depth == 0 && bracket_depth == 0 => {
                selectors.push(&serialized[start..index]);
                start = index + 1;
            }
            _ => {}
        }

        index += 1;
    }

    selectors.push(&serialized[start..]);
    selectors
}

fn serialize_component<'a, 'i, W>(
    component: &Component<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
    selector_source: Option<&str>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    match component {
        Component::Combinator(c) => c.to_css(dest),
        Component::AttributeInNoNamespace {
            local_name,
            operator,
            value,
            case_sensitivity,
            ..
        } => {
            dest.write_char('[')?;
            cssparser::ToCss::to_css(local_name, dest)?;
            cssparser::ToCss::to_css(operator, dest)?;

            if dest.minify && !dest.is_react_compat() {
                // Serialize as both an identifier and a string and choose the shorter one.
                let mut id = String::new();
                serialize_identifier(&value, &mut id)?;

                let s = value.to_css_string(Default::default())?;

                if id.len() > 0 && id.len() < s.len() {
                    dest.write_str(&id)?;
                } else {
                    dest.write_str(&s)?;
                }
            } else {
                value.to_css(dest)?;
            }

            match case_sensitivity {
        crate::parcel_selectors::attr::ParsedCaseSensitivity::CaseSensitive
        | crate::parcel_selectors::attr::ParsedCaseSensitivity::AsciiCaseInsensitiveIfInHtmlElementInHtmlDocument => {}
        crate::parcel_selectors::attr::ParsedCaseSensitivity::AsciiCaseInsensitive => dest.write_str(" i")?,
        crate::parcel_selectors::attr::ParsedCaseSensitivity::ExplicitCaseSensitive => dest.write_str(" s")?,
      }
            dest.write_char(']')
        }
        Component::Is(list)
        | Component::Where(list)
        | Component::Negation(list)
        | Component::Any(_, list) => {
            match *component {
                Component::Where(..) => dest.write_str(":where(")?,
                Component::Is(ref selectors) => {
                    // If there's only one simple selector, serialize it directly.
                    if should_unwrap_is(selectors) {
                        serialize_selector(selectors.first().unwrap(), dest, context, false, None)?;
                        return Ok(());
                    }

                    let vp = dest.vendor_prefix;
                    if vp.intersects(VendorPrefix::WebKit | VendorPrefix::Moz) {
                        dest.write_char(':')?;
                        vp.to_css(dest)?;
                        dest.write_str("any(")?;
                    } else {
                        dest.write_str(":is(")?;
                    }
                }
                Component::Negation(_) => {
                    dest.write_str(":not(")?;
                }
                Component::Any(prefix, ..) => {
                    let vp = dest.vendor_prefix.or(prefix);
                    if vp.intersects(VendorPrefix::WebKit | VendorPrefix::Moz) {
                        dest.write_char(':')?;
                        vp.to_css(dest)?;
                        dest.write_str("any(")?;
                    } else {
                        dest.write_str(":is(")?;
                    }
                }
                _ => unreachable!(),
            }
            serialize_selector_list(list.iter(), dest, context, false, None)?;
            dest.write_str(")")
        }
        Component::Has(list) => {
            dest.write_str(":has(")?;
            serialize_selector_list(list.iter(), dest, context, true, None)?;
            dest.write_str(")")
        }
        Component::Nth(nth_data) => serialize_nth(nth_data, dest, selector_source),
        Component::NonTSPseudoClass(pseudo) => serialize_pseudo_class(pseudo, dest, context),
        Component::PseudoElement(pseudo) => {
            serialize_pseudo_element(pseudo, dest, context, selector_source)
        }
        Component::Nesting => serialize_nesting(dest, context, false),
        Component::Class(class) => {
            dest.write_char('.')?;
            dest.write_ident(&class.0, true)
        }
        Component::ID(id) => {
            dest.write_char('#')?;
            dest.write_ident(&id.0, true)
        }
        Component::Host(selector) => {
            dest.write_str(":host")?;
            if let Some(ref selector) = *selector {
                dest.write_char('(')?;
                serialize_selector(selector, dest, context, false, None)?;
                dest.write_char(')')?;
            }
            Ok(())
        }
        Component::Slotted(selector) => {
            dest.write_str("::slotted(")?;
            serialize_selector(selector, dest, context, false, None)?;
            dest.write_char(')')
        }
        Component::NthOf(nth_of_data) => {
            let nth_data = nth_of_data.nth_data();
            nth_data.write_start(dest, true)?;
            nth_data.write_affine(dest)?;
            dest.write_str(" of ")?;
            serialize_selector_list(nth_of_data.selectors().iter(), dest, context, true, None)?;
            dest.write_char(')')
        }
        _ => {
            cssparser::ToCss::to_css(component, dest)?;
            Ok(())
        }
    }
}

fn serialize_nth<W>(
    nth_data: &NthSelectorData,
    dest: &mut Printer<W>,
    selector_source: Option<&str>,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    let preserve_authored_function = dest.is_react_compat()
        && nth_data.a == 0
        && nth_data.b == 1
        && selector_source.is_some_and(|source| match nth_data.ty {
            NthType::Child => source.contains(":nth-child("),
            NthType::LastChild => source.contains(":nth-last-child("),
            NthType::OfType => source.contains(":nth-of-type("),
            NthType::LastOfType => source.contains(":nth-last-of-type("),
            _ => false,
        });

    let is_function = nth_data.is_function() || preserve_authored_function;
    nth_data.write_start(dest, is_function)?;
    if is_function {
        nth_data.write_affine(dest)?;
        dest.write_char(')')?;
    }

    Ok(())
}

fn should_unwrap_is<'i>(selectors: &Box<[Selector<'i>]>) -> bool {
    if selectors.len() == 1 {
        let first = selectors.first().unwrap();
        if !has_type_selector(first) && is_simple(first) {
            return true;
        }
    }

    false
}

fn serialize_nesting<W>(
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, '_>>,
    first: bool,
) -> Result<(), PrinterError>
where
    W: fmt::Write,
{
    if let Some(ctx) = context {
        let serialize_as_relative = first
            && dest.is_react_compat()
            && ctx.selectors.0.len() == 1
            && selector_has_leading_scope(&ctx.selectors.0[0]);

        // If there's only one selector and it does not start with a type selector,
        // React/stylis serializes it directly even when the selector is complex.
        // Keep the stricter simple-selector behavior for standards mode.
        if ctx.selectors.0.len() == 1
            && (first
                || (!has_type_selector(&ctx.selectors.0[0])
                    && (is_simple(&ctx.selectors.0[0]) || dest.is_react_compat())))
        {
            serialize_selector(
                ctx.selectors.0.first().unwrap(),
                dest,
                ctx.parent,
                serialize_as_relative,
                ctx.selector_source,
            )
        } else {
            dest.write_str(":is(")?;
            serialize_selector_list(
                ctx.selectors.0.iter(),
                dest,
                ctx.parent,
                false,
                ctx.selector_source,
            )?;
            dest.write_char(')')
        }
    } else {
        // If there is no context, we are at the root if nesting is supported. This is equivalent to :scope.
        // Otherwise, if nesting is supported, serialize the nesting selector directly.
        if should_compile!(dest.targets.current, Nesting) {
            dest.write_str(":scope")
        } else {
            dest.write_char('&')
        }
    }
}

#[inline]
fn has_type_selector(selector: &Selector) -> bool {
    // For input:checked the component vector is
    // [input, :checked] so we have to check it using matching order.
    //
    // This both happens for input:checked and is(input:checked)
    let mut iter = selector.iter_raw_match_order();
    let first = iter.next();

    if is_namespace(first) {
        is_type_selector(iter.next())
    } else {
        is_type_selector(first)
    }
}

#[inline]
fn has_nesting(selector: &Selector) -> bool {
    selector
        .iter_raw_match_order()
        .any(|component| matches!(component, Component::Nesting))
}

#[inline]
fn selector_has_leading_scope(selector: &Selector) -> bool {
    let combinator_count = selector
        .iter_raw_match_order()
        .filter(|component| component.is_combinator())
        .count();
    let compound_count = combinator_count + 1;

    selector
        .iter_raw_match_order()
        .as_slice()
        .split(|component| component.is_combinator())
        .rev()
        .take(compound_count)
        .next()
        .is_some_and(|compound| matches!(compound.first(), Some(Component::Scope)))
}

#[inline]
fn is_simple(selector: &Selector) -> bool {
    !selector
        .iter_raw_match_order()
        .any(|component| component.is_combinator())
}

#[inline]
fn is_type_selector(component: Option<&Component>) -> bool {
    matches!(
        component,
        Some(Component::LocalName(_)) | Some(Component::ExplicitUniversalType)
    )
}

#[inline]
fn is_suffix_nesting_type_selector(component: Option<&Component>) -> bool {
    matches!(
        component,
        Some(Component::LocalName(local)) if local.name.0.as_ref().starts_with('-')
    )
}

#[inline]
fn is_namespace(component: Option<&Component>) -> bool {
    matches!(
        component,
        Some(Component::ExplicitAnyNamespace)
            | Some(Component::ExplicitNoNamespace)
            | Some(Component::Namespace(..))
            | Some(Component::DefaultNamespace(_))
    )
}

fn serialize_selector_list<'a, 'i: 'a, I, W>(
    iter: I,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
    is_relative: bool,
    selector_source_list: Option<&str>,
) -> Result<(), PrinterError>
where
    I: Iterator<Item = &'a Selector<'i>>,
    W: fmt::Write,
{
    let authored_branches = selector_source_list.map(split_top_level_selector_list);
    let mut first = true;
    for (index, selector) in iter.enumerate() {
        if !first {
            dest.delim(',', false)?;
        }
        first = false;
        let selector_source = authored_branches
            .as_ref()
            .and_then(|branches| branches.get(index))
            .copied();
        serialize_selector(selector, dest, context, is_relative, selector_source)?;
    }
    Ok(())
}

fn serialize_selector_for_namespace<'i, W>(
    selector: &Selector<'i>,
    dest: &mut Printer<W>,
    context: Option<&StyleContext<'_, 'i>>,
) -> Result<String, PrinterError>
where
    W: fmt::Write,
{
    let mut output = String::new();
    let mut printer = Printer::new(
        &mut output,
        PrinterOptions {
            minify: true,
            targets: dest.targets.current,
            normalization_profile: dest.normalization_profile,
            namespace_middleware: false,
            ..PrinterOptions::default()
        },
    );
    serialize_selector(selector, &mut printer, context, false, None)?;
    Ok(output)
}

fn namespace_selector(selector: &str) -> String {
    let mut tokens = namespace_tokenize(selector);
    let len = tokens.len();
    let mut element = String::new();
    let mut output = String::new();

    for original_index in 0..len {
        let value = tokens[original_index].clone();
        let mut index = original_index;
        let piece = match namespace_char_code(&value) {
            12 => value[1..].to_owned(),
            0 | 40 | 43 | 62 | 126 => value,
            58 => {
                index += 1;
                if tokens.get(index).is_some_and(|token| token == "global") {
                    tokens[index].clear();
                    let value_index = index + 1;
                    if let Some(source) = tokens.get(value_index).cloned() {
                        let inner = source
                            .strip_prefix('(')
                            .and_then(|value| value.strip_suffix(')'))
                            .unwrap_or_default();
                        tokens[value_index] = format!("\u{c}{inner}");
                    }
                    index = 1;
                }

                if index == 1 { String::new() } else { value }
            }
            32 => {
                if index == 1 {
                    String::new()
                } else {
                    value
                }
            }
            _ => match index {
                0 => {
                    element = value.clone();
                    if len > 1 { String::new() } else { value }
                }
                _ if index == len - 1 || index == 2 => {
                    if index == 2 {
                        format!("{value}{element}{element}")
                    } else {
                        format!("{value}{element}")
                    }
                }
                _ => value,
            },
        };
        output.push_str(&piece);
    }

    output
}

fn namespace_tokenize(input: &str) -> Vec<String> {
    let mut tokenizer = NamespaceTokenizer::new(input);
    let mut tokens = Vec::new();

    while let Some(character) = tokenizer.next() {
        match namespace_token(character) {
            0 => tokens.push(tokenizer.identifier(tokenizer.position - 1).to_owned()),
            2 => tokens.push(tokenizer.delimit(character)),
            _ => tokens.push(char::from(character).to_string()),
        }
    }

    tokens
}

fn namespace_char_code(value: &str) -> u8 {
    value.as_bytes().first().copied().unwrap_or(0)
}

fn namespace_token(value: u8) -> u8 {
    match value {
        0 | 9 | 10 | 13 | 32 => 5,
        33 | 43 | 44 | 47 | 62 | 64 | 126 | 59 | 123 | 125 => 4,
        58 => 3,
        34 | 39 | 40 | 91 => 2,
        41 | 93 => 1,
        _ => 0,
    }
}

struct NamespaceTokenizer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    position: usize,
}

impl<'a> NamespaceTokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            position: 0,
        }
    }

    fn next(&mut self) -> Option<u8> {
        let value = self.bytes.get(self.position).copied()?;
        self.position += 1;
        Some(value)
    }

    fn peek(&self) -> u8 {
        self.bytes.get(self.position).copied().unwrap_or(0)
    }

    fn slice(&self, begin: usize, end: usize) -> &str {
        &self.input[begin..end]
    }

    fn identifier(&mut self, start: usize) -> &str {
        while namespace_token(self.peek()) == 0 {
            if self.next().is_none() {
                break;
            }
        }
        self.slice(start, self.position)
    }

    fn delimit(&mut self, open: u8) -> String {
        let start = self.position - 1;
        let end = self.delimiter(match open {
            b'[' => b']',
            b'(' => b')',
            _ => open,
        });
        self.slice(start, end).trim().to_owned()
    }

    fn delimiter(&mut self, close: u8) -> usize {
        while let Some(character) = self.next() {
            match character {
                current if current == close => return self.position,
                b'"' | b'\'' if close != b'"' && close != b'\'' => {
                    let nested_close = character;
                    self.delimiter(nested_close);
                }
                b'(' if close == b')' => {
                    self.delimiter(close);
                }
                b'\\' => {
                    let _ = self.next();
                }
                _ => {}
            }
        }

        self.position
    }
}

pub(crate) fn is_compatible(selectors: &[Selector], targets: Targets) -> bool {
    for selector in selectors {
        let iter = selector.iter_raw_match_order();
        for component in iter {
            let feature = match component {
                Component::ID(_) | Component::Class(_) | Component::LocalName(_) => continue,

                Component::ExplicitAnyNamespace
                | Component::ExplicitNoNamespace
                | Component::DefaultNamespace(_)
                | Component::Namespace(_, _) => Feature::Namespaces,

                Component::ExplicitUniversalType => Feature::Selectors2,

                Component::AttributeInNoNamespaceExists { .. } => Feature::Selectors2,
                Component::AttributeInNoNamespace {
                    operator,
                    case_sensitivity,
                    ..
                } => {
                    if *case_sensitivity != ParsedCaseSensitivity::CaseSensitive {
                        Feature::CaseInsensitive
                    } else {
                        match operator {
                            AttrSelectorOperator::Equal
                            | AttrSelectorOperator::Includes
                            | AttrSelectorOperator::DashMatch => Feature::Selectors2,
                            AttrSelectorOperator::Prefix
                            | AttrSelectorOperator::Substring
                            | AttrSelectorOperator::Suffix => Feature::Selectors3,
                        }
                    }
                }
                Component::AttributeOther(attr) => match attr.operation {
                    ParsedAttrSelectorOperation::Exists => Feature::Selectors2,
                    ParsedAttrSelectorOperation::WithValue {
                        operator,
                        case_sensitivity,
                        ..
                    } => {
                        if case_sensitivity != ParsedCaseSensitivity::CaseSensitive {
                            Feature::CaseInsensitive
                        } else {
                            match operator {
                                AttrSelectorOperator::Equal
                                | AttrSelectorOperator::Includes
                                | AttrSelectorOperator::DashMatch => Feature::Selectors2,
                                AttrSelectorOperator::Prefix
                                | AttrSelectorOperator::Substring
                                | AttrSelectorOperator::Suffix => Feature::Selectors3,
                            }
                        }
                    }
                },

                Component::Empty | Component::Root => Feature::Selectors3,
                Component::Negation(selectors) => {
                    // :not() selector list is not forgiving.
                    if !targets.is_compatible(Feature::Selectors3)
                        || !is_compatible(&*selectors, targets)
                    {
                        return false;
                    }
                    continue;
                }

                Component::Nth(data) => match data.ty {
                    NthType::Child if data.a == 0 && data.b == 1 => Feature::Selectors2,
                    NthType::Col | NthType::LastCol => return false,
                    _ => Feature::Selectors3,
                },
                Component::NthOf(n) => {
                    if !targets.is_compatible(Feature::NthChildOf)
                        || !is_compatible(n.selectors(), targets)
                    {
                        return false;
                    }
                    continue;
                }

                // These support forgiving selector lists, so no need to check nested selectors.
                Component::Is(selectors) => {
                    // ... except if we are going to unwrap them.
                    if should_unwrap_is(selectors) && is_compatible(selectors, targets) {
                        continue;
                    }

                    Feature::IsSelector
                }
                Component::Where(_) | Component::Nesting => Feature::IsSelector,
                Component::Any(..) => return false,
                Component::Has(selectors) => {
                    if !targets.is_compatible(Feature::HasSelector)
                        || !is_compatible(&*selectors, targets)
                    {
                        return false;
                    }
                    continue;
                }

                Component::Scope | Component::Host(_) | Component::Slotted(_) => {
                    Feature::Shadowdomv1
                }

                Component::Part(_) => Feature::PartPseudo,

                Component::NonTSPseudoClass(pseudo) => {
                    match pseudo {
                        PseudoClass::Link
                        | PseudoClass::Visited
                        | PseudoClass::Active
                        | PseudoClass::Hover
                        | PseudoClass::Focus
                        | PseudoClass::Lang { languages: _ } => Feature::Selectors2,

                        PseudoClass::Checked
                        | PseudoClass::Disabled
                        | PseudoClass::Enabled
                        | PseudoClass::Target => Feature::Selectors3,

                        PseudoClass::AnyLink(prefix) if *prefix == VendorPrefix::None => {
                            Feature::AnyLink
                        }
                        PseudoClass::Indeterminate => Feature::IndeterminatePseudo,

                        PseudoClass::Fullscreen(prefix) if *prefix == VendorPrefix::None => {
                            Feature::Fullscreen
                        }

                        PseudoClass::FocusVisible => Feature::FocusVisible,
                        PseudoClass::FocusWithin => Feature::FocusWithin,
                        PseudoClass::Default => Feature::DefaultPseudo,
                        PseudoClass::Dir { direction: _ } => Feature::DirSelector,
                        PseudoClass::Optional => Feature::OptionalPseudo,
                        PseudoClass::PlaceholderShown(prefix) if *prefix == VendorPrefix::None => {
                            Feature::PlaceholderShown
                        }

                        PseudoClass::ReadOnly(prefix) | PseudoClass::ReadWrite(prefix)
                            if *prefix == VendorPrefix::None =>
                        {
                            Feature::ReadOnlyWrite
                        }

                        PseudoClass::Valid | PseudoClass::Invalid | PseudoClass::Required => {
                            Feature::FormValidation
                        }

                        PseudoClass::InRange | PseudoClass::OutOfRange => Feature::InOutOfRange,

                        PseudoClass::Autofill(prefix) if *prefix == VendorPrefix::None => {
                            Feature::Autofill
                        }

                        PseudoClass::State { .. } => Feature::StatePseudoClass,

                        // Experimental, no browser support.
                        PseudoClass::Current
                        | PseudoClass::Past
                        | PseudoClass::Future
                        | PseudoClass::Playing
                        | PseudoClass::Paused
                        | PseudoClass::Seeking
                        | PseudoClass::Stalled
                        | PseudoClass::Buffering
                        | PseudoClass::Muted
                        | PseudoClass::VolumeLocked
                        | PseudoClass::TargetWithin
                        | PseudoClass::LocalLink
                        | PseudoClass::Blank
                        | PseudoClass::UserInvalid
                        | PseudoClass::UserValid
                        | PseudoClass::Defined
                        | PseudoClass::ActiveViewTransition
                        | PseudoClass::ActiveViewTransitionType { .. } => return false,

                        PseudoClass::Custom { .. } | _ => return false,
                    }
                }

                Component::PseudoElement(pseudo) => match pseudo {
                    PseudoElement::After | PseudoElement::Before => Feature::Gencontent,
                    PseudoElement::FirstLine => Feature::FirstLine,
                    PseudoElement::FirstLetter => Feature::FirstLetter,
                    PseudoElement::DetailsContent => Feature::DetailsContent,
                    PseudoElement::TargetText => Feature::TargetText,
                    PseudoElement::Selection(prefix) if *prefix == VendorPrefix::None => {
                        Feature::Selection
                    }
                    PseudoElement::Placeholder(prefix) if *prefix == VendorPrefix::None => {
                        Feature::Placeholder
                    }
                    PseudoElement::Marker => Feature::MarkerPseudo,
                    PseudoElement::Backdrop(prefix) if *prefix == VendorPrefix::None => {
                        Feature::Dialog
                    }
                    PseudoElement::Cue => Feature::Cue,
                    PseudoElement::CueFunction { selector: _ } => Feature::CueFunction,
                    PseudoElement::ViewTransition
                    | PseudoElement::ViewTransitionNew { .. }
                    | PseudoElement::ViewTransitionOld { .. }
                    | PseudoElement::ViewTransitionGroup { .. }
                    | PseudoElement::ViewTransitionImagePair { .. } => Feature::ViewTransition,
                    PseudoElement::PickerFunction { identifier: _ } => Feature::Picker,
                    PseudoElement::PickerIcon => Feature::PickerIcon,
                    PseudoElement::Checkmark => Feature::Checkmark,
                    PseudoElement::GrammarError => Feature::GrammarError,
                    PseudoElement::SpellingError => Feature::SpellingError,
                    PseudoElement::Custom { name: _ } | _ => return false,
                },

                Component::Combinator(combinator) => match combinator {
                    Combinator::Child | Combinator::NextSibling => Feature::Selectors2,
                    Combinator::LaterSibling => Feature::Selectors3,
                    _ => continue,
                },
            };

            if !targets.is_compatible(feature) {
                return false;
            }
        }
    }

    true
}

/// Returns whether two selector lists are equivalent, i.e. the same minus any vendor prefix differences.
pub(crate) fn is_equivalent<'i>(selectors: &[Selector<'i>], other: &[Selector<'i>]) -> bool {
    if selectors.len() != other.len() {
        return false;
    }

    for (i, a) in selectors.iter().enumerate() {
        let b = &other[i];
        if a.len() != b.len() {
            return false;
        }

        for (a, b) in a.iter_raw_match_order().zip(b.iter_raw_match_order()) {
            let is_equivalent = match (a, b) {
                (Component::NonTSPseudoClass(a_ps), Component::NonTSPseudoClass(b_ps)) => {
                    a_ps.is_equivalent(b_ps)
                }
                (Component::PseudoElement(a_pe), Component::PseudoElement(b_pe)) => {
                    a_pe.is_equivalent(b_pe)
                }
                (Component::Any(_, a), Component::Is(b))
                | (Component::Is(a), Component::Any(_, b))
                | (Component::Any(_, a), Component::Any(_, b))
                | (Component::Is(a), Component::Is(b)) => is_equivalent(&*a, &*b),
                (a, b) => a == b,
            };

            if !is_equivalent {
                return false;
            }
        }
    }

    true
}

/// Returns the vendor prefix (if any) used in the given selector list.
/// If multiple vendor prefixes are seen, this is invalid, and an empty result is returned.
pub(crate) fn get_prefix(selectors: &SelectorList) -> VendorPrefix {
    let mut prefix = VendorPrefix::empty();
    for selector in &selectors.0 {
        for component in selector.iter_raw_match_order() {
            let p = match component {
                // Return none rather than empty for these so that we call downlevel_selectors.
                Component::NonTSPseudoClass(PseudoClass::Lang { .. })
                | Component::NonTSPseudoClass(PseudoClass::Dir { .. })
                | Component::Is(..)
                | Component::Where(..)
                | Component::Has(..)
                | Component::Negation(..) => VendorPrefix::None,
                Component::Any(prefix, _) => *prefix,
                Component::NonTSPseudoClass(pc) => pc.get_prefix(),
                Component::PseudoElement(pe) => pe.get_prefix(),
                _ => VendorPrefix::empty(),
            };

            if !p.is_empty() {
                // Allow none to be mixed with a prefix.
                let prefix_without_none = prefix - VendorPrefix::None;
                if prefix_without_none.is_empty() || prefix_without_none == p {
                    prefix |= p;
                } else {
                    return VendorPrefix::empty();
                }
            }
        }
    }

    prefix
}

const RTL_LANGS: &[&str] = &[
    "ae", "ar", "arc", "bcc", "bqi", "ckb", "dv", "fa", "glk", "he", "ku", "mzn", "nqo", "pnb",
    "ps", "sd", "ug", "ur", "yi",
];

/// Downlevels the given selectors to be compatible with the given browser targets.
/// Returns the necessary vendor prefixes.
pub(crate) fn downlevel_selectors(selectors: &mut [Selector], targets: Targets) -> VendorPrefix {
    let mut necessary_prefixes = VendorPrefix::empty();
    for selector in selectors {
        for component in selector.iter_mut_raw_match_order() {
            necessary_prefixes |= downlevel_component(component, targets);
        }
    }

    necessary_prefixes
}

pub(crate) fn react_compat_prefix_selectors(selectors: &mut [Selector]) -> VendorPrefix {
    fn visit<'i>(component: &mut Component<'i>) -> VendorPrefix {
        match component {
            Component::NonTSPseudoClass(PseudoClass::ReadOnly(prefix))
            | Component::NonTSPseudoClass(PseudoClass::ReadWrite(prefix)) => {
                if *prefix == VendorPrefix::None {
                    *prefix = VendorPrefix::Moz | VendorPrefix::None;
                }
                *prefix
            }
            Component::PseudoElement(PseudoElement::Placeholder(prefix)) => {
                if *prefix == VendorPrefix::None {
                    *prefix = VendorPrefix::WebKit
                        | VendorPrefix::Moz
                        | VendorPrefix::Ms
                        | VendorPrefix::None;
                }
                *prefix
            }
            Component::Any(_, selectors)
            | Component::Is(selectors)
            | Component::Where(selectors)
            | Component::Has(selectors)
            | Component::Negation(selectors) => react_compat_prefix_selectors(selectors),
            _ => VendorPrefix::empty(),
        }
    }

    let mut prefixes = VendorPrefix::empty();
    for selector in selectors {
        for component in selector.iter_mut_raw_match_order() {
            prefixes |= visit(component);
        }
    }
    prefixes
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReactCompatSelectorMiddlewareKind {
    Placeholder,
    ReadOnly,
    ReadWrite,
}

pub(crate) struct ReactCompatSelectorMiddlewareSplit<'i> {
    pub base: Option<SelectorList<'i>>,
    pub lifted: Vec<SelectorList<'i>>,
}

pub(crate) fn react_compat_selector_middleware_split<'i>(
    selectors: &SelectorList<'i>,
) -> Option<ReactCompatSelectorMiddlewareSplit<'i>> {
    let mut base = SmallVec::new();
    let mut generated = Vec::new();

    for selector in &selectors.0 {
        if let Some(kind) = selector_middleware_kind(selector) {
            generated.extend(selector_middleware_variants(selector, kind));
        } else {
            base.push(selector.clone());
        }
    }

    if generated.is_empty() {
        return None;
    }

    Some(ReactCompatSelectorMiddlewareSplit {
        base: (!base.is_empty()).then(|| crate::parcel_selectors::SelectorList(base)),
        lifted: generated.into_iter().map(SelectorList::from).collect(),
    })
}

fn selector_middleware_kind(selector: &Selector) -> Option<ReactCompatSelectorMiddlewareKind> {
    for component in selector.iter_raw_match_order() {
        if let Some(kind) = component_middleware_kind(component) {
            return Some(kind);
        }
    }
    None
}

fn component_middleware_kind(component: &Component) -> Option<ReactCompatSelectorMiddlewareKind> {
    let placeholder_prefixes =
        VendorPrefix::WebKit | VendorPrefix::Moz | VendorPrefix::Ms | VendorPrefix::None;
    let read_prefixes = VendorPrefix::Moz | VendorPrefix::None;

    match component {
        Component::PseudoElement(PseudoElement::Placeholder(prefix))
            if *prefix == placeholder_prefixes =>
        {
            Some(ReactCompatSelectorMiddlewareKind::Placeholder)
        }
        Component::NonTSPseudoClass(PseudoClass::ReadOnly(prefix)) if *prefix == read_prefixes => {
            Some(ReactCompatSelectorMiddlewareKind::ReadOnly)
        }
        Component::NonTSPseudoClass(PseudoClass::ReadWrite(prefix)) if *prefix == read_prefixes => {
            Some(ReactCompatSelectorMiddlewareKind::ReadWrite)
        }
        Component::Any(_, selectors)
        | Component::Is(selectors)
        | Component::Where(selectors)
        | Component::Has(selectors)
        | Component::Negation(selectors) => selectors.iter().find_map(selector_middleware_kind),
        _ => None,
    }
}

fn selector_middleware_variants<'i>(
    selector: &Selector<'i>,
    kind: ReactCompatSelectorMiddlewareKind,
) -> Vec<Selector<'i>> {
    let prefixes = match kind {
        ReactCompatSelectorMiddlewareKind::Placeholder => {
            vec![
                VendorPrefix::WebKit,
                VendorPrefix::Moz,
                VendorPrefix::Ms,
                VendorPrefix::None,
            ]
        }
        ReactCompatSelectorMiddlewareKind::ReadOnly
        | ReactCompatSelectorMiddlewareKind::ReadWrite => {
            vec![VendorPrefix::Moz, VendorPrefix::None]
        }
    };

    prefixes
        .into_iter()
        .map(|prefix| {
            let mut selector = selector.clone();
            let updated = apply_selector_middleware_prefix(&mut selector, kind, prefix);
            debug_assert!(updated);
            selector
        })
        .collect()
}

fn apply_selector_middleware_prefix(
    selector: &mut Selector,
    kind: ReactCompatSelectorMiddlewareKind,
    prefix: VendorPrefix,
) -> bool {
    for component in selector.iter_mut_raw_match_order() {
        if apply_component_middleware_prefix(component, kind, prefix) {
            return true;
        }
    }

    false
}

fn apply_component_middleware_prefix(
    component: &mut Component,
    kind: ReactCompatSelectorMiddlewareKind,
    prefix: VendorPrefix,
) -> bool {
    match component {
        Component::PseudoElement(PseudoElement::Placeholder(current))
            if kind == ReactCompatSelectorMiddlewareKind::Placeholder =>
        {
            *current = prefix;
            true
        }
        Component::NonTSPseudoClass(PseudoClass::ReadOnly(current))
            if kind == ReactCompatSelectorMiddlewareKind::ReadOnly =>
        {
            *current = prefix;
            true
        }
        Component::NonTSPseudoClass(PseudoClass::ReadWrite(current))
            if kind == ReactCompatSelectorMiddlewareKind::ReadWrite =>
        {
            *current = prefix;
            true
        }
        Component::Any(_, selectors)
        | Component::Is(selectors)
        | Component::Where(selectors)
        | Component::Has(selectors)
        | Component::Negation(selectors) => selectors
            .iter_mut()
            .any(|selector| apply_selector_middleware_prefix(selector, kind, prefix)),
        _ => false,
    }
}

fn downlevel_component<'i>(component: &mut Component<'i>, targets: Targets) -> VendorPrefix {
    match component {
        Component::NonTSPseudoClass(pc) => {
            match pc {
                PseudoClass::Dir { direction: dir } => {
                    if should_compile!(targets, DirSelector) {
                        *component = downlevel_dir(*dir, targets);
                        downlevel_component(component, targets)
                    } else {
                        VendorPrefix::empty()
                    }
                }
                PseudoClass::Lang { languages: langs } => {
                    // :lang() with multiple languages is not supported everywhere.
                    // compile this to :is(:lang(a), :lang(b)) etc.
                    if langs.len() > 1 && should_compile!(targets, LangSelectorList) {
                        *component = Component::Is(lang_list_to_selectors(&langs));
                        downlevel_component(component, targets)
                    } else {
                        VendorPrefix::empty()
                    }
                }
                _ => pc.get_necessary_prefixes(targets),
            }
        }
        Component::PseudoElement(pe) => pe.get_necessary_prefixes(targets),
        Component::Is(selectors) => {
            let mut necessary_prefixes = downlevel_selectors(&mut **selectors, targets);

            // Convert :is to :-webkit-any/:-moz-any if needed.
            // All selectors must be simple, no combinators are supported.
            if should_compile!(targets, IsSelector)
                && !should_unwrap_is(selectors)
                && selectors.iter().all(|selector| !selector.has_combinator())
            {
                necessary_prefixes |=
                    targets.prefixes(VendorPrefix::None, crate::prefixes::Feature::AnyPseudo)
            } else {
                necessary_prefixes |= VendorPrefix::None
            }

            necessary_prefixes
        }
        Component::Negation(selectors) => {
            let mut necessary_prefixes = downlevel_selectors(&mut **selectors, targets);

            // Downlevel :not(.a, .b) -> :not(:is(.a, .b)) if not list is unsupported.
            // We need to use :is() / :-webkit-any() rather than :not(.a):not(.b) to ensure the specificity is equivalent.
            // https://drafts.csswg.org/selectors/#specificity-rules
            if selectors.len() > 1 && should_compile!(targets, NotSelectorList) {
                *component = Component::Negation(
                    vec![Selector::from(Component::Is(selectors.clone()))].into_boxed_slice(),
                );

                if should_compile!(targets, IsSelector) {
                    necessary_prefixes |=
                        targets.prefixes(VendorPrefix::None, crate::prefixes::Feature::AnyPseudo)
                } else {
                    necessary_prefixes |= VendorPrefix::None
                }
            }

            necessary_prefixes
        }
        Component::Where(selectors) | Component::Any(_, selectors) | Component::Has(selectors) => {
            downlevel_selectors(&mut **selectors, targets)
        }
        _ => VendorPrefix::empty(),
    }
}

fn lang_list_to_selectors<'i>(langs: &Vec<CowArcStr<'i>>) -> Box<[Selector<'i>]> {
    langs
        .iter()
        .map(|lang| {
            Selector::from(Component::NonTSPseudoClass(PseudoClass::Lang {
                languages: vec![lang.clone()],
            }))
        })
        .collect::<Vec<Selector>>()
        .into_boxed_slice()
}

fn downlevel_dir<'i>(dir: Direction, targets: Targets) -> Component<'i> {
    // Convert :dir to :lang. If supported, use a list of languages in a single :lang,
    // otherwise, use :is/:not, which may be further downleveled to e.g. :-webkit-any.
    let langs = RTL_LANGS.iter().map(|lang| (*lang).into()).collect();
    if !should_compile!(targets, LangSelectorList) {
        let c = Component::NonTSPseudoClass(PseudoClass::Lang { languages: langs });
        if dir == Direction::Ltr {
            Component::Negation(vec![Selector::from(c)].into_boxed_slice())
        } else {
            c
        }
    } else {
        if dir == Direction::Ltr {
            Component::Negation(lang_list_to_selectors(&langs))
        } else {
            Component::Is(lang_list_to_selectors(&langs))
        }
    }
}

/// Determines whether a selector list contains only unused selectors.
/// A selector is considered unused if it contains a class or id component that exists in the set of unused symbols.
pub(crate) fn is_unused(
    selectors: &mut std::slice::Iter<Selector>,
    unused_symbols: &HashSet<String>,
    parent_is_unused: bool,
) -> bool {
    if unused_symbols.is_empty() {
        return false;
    }

    selectors.all(|selector| {
        for component in selector.iter_raw_match_order() {
            match component {
                Component::Class(name) | Component::ID(name) => {
                    if unused_symbols.contains(&name.0.to_string()) {
                        return true;
                    }
                }
                Component::Is(is) | Component::Where(is) | Component::Any(_, is) => {
                    if is_unused(&mut is.iter(), unused_symbols, parent_is_unused) {
                        return true;
                    }
                }
                Component::Nesting => {
                    if parent_is_unused {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    })
}

/// Returns whether the selector has any class or id components.
pub(crate) fn is_pure_css_modules_selector(selector: &Selector) -> bool {
    use crate::parcel_selectors::parser::Component;
    selector.iter_raw_match_order().any(|c| match c {
        Component::Class(_) | Component::ID(_) => true,
        Component::Is(s)
        | Component::Where(s)
        | Component::Has(s)
        | Component::Any(_, s)
        | Component::Negation(s) => s.iter().any(is_pure_css_modules_selector),
        Component::NthOf(nth) => nth.selectors().iter().any(is_pure_css_modules_selector),
        Component::Slotted(s) => is_pure_css_modules_selector(&s),
        Component::Host(s) => s
            .as_ref()
            .map(is_pure_css_modules_selector)
            .unwrap_or(false),
        Component::NonTSPseudoClass(pc) => match pc {
            PseudoClass::Local { selector } => is_pure_css_modules_selector(&*selector),
            _ => false,
        },
        _ => false,
    })
}

impl<'i> ParseWithOptions<'i> for Selector<'i> {
    fn parse_with_options<'t>(
        input: &mut Parser<'i, 't>,
        options: &ParserOptions<'_, 'i>,
    ) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        Selector::parse(
            &SelectorParser {
                is_nesting_allowed: true,
                options: &options,
            },
            input,
        )
    }
}

impl<'i> ParseWithOptions<'i> for SelectorList<'i> {
    fn parse_with_options<'t>(
        input: &mut Parser<'i, 't>,
        options: &ParserOptions<'_, 'i>,
    ) -> Result<Self, ParseError<'i, ParserError<'i>>> {
        SelectorList::parse(
            &SelectorParser {
                is_nesting_allowed: true,
                options: &options,
            },
            input,
            crate::parcel_selectors::parser::ParseErrorRecovery::DiscardList,
            crate::parcel_selectors::parser::NestingRequirement::None,
        )
    }
}
