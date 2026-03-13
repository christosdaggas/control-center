Name:           lnx-control-center
Version:        1.5.0
Release:        1%{?dist}
Summary:        Linux System Change & Activity Timeline Viewer

License:        MIT
URL:            https://chrisdaggas.com
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  gtk4-devel >= 4.12
BuildRequires:  libadwaita-devel >= 1.4

Requires:       gtk4 >= 4.12
Requires:       libadwaita >= 1.4
Requires:       systemd

%description
Control Center is a GTK4/libadwaita application that presents a unified 
timeline of meaningful system changes, turning logs and events into an 
understandable narrative.

Features:
- Unified Timeline: Aggregate events from journald, package managers, kernel
- Smart Filtering: Preset filters for common use cases
- Correlation Engine: Rule-based grouping of related events
- Human-Readable Summaries: Clear explanations with expandable raw evidence

%prep
%autosetup

%build
cargo build --release

%install
install -Dm755 target/release/control-center %{buildroot}%{_bindir}/control-center
install -Dm644 data/com.chrisdaggas.control-center.desktop %{buildroot}%{_datadir}/applications/com.chrisdaggas.control-center.desktop
install -Dm644 data/com.chrisdaggas.control-center.metainfo.xml %{buildroot}%{_metainfodir}/com.chrisdaggas.control-center.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/control-center
%{_datadir}/applications/com.chrisdaggas.control-center.desktop
%{_metainfodir}/com.chrisdaggas.control-center.metainfo.xml

%changelog
* Fri Mar 13 2026 Christos A. Daggas <info@chrisdaggas.com> - 1.5.0-1
- Add Security Posture page and security snapshot comparison support

* Mon Jan 13 2026 Christos A. Daggas <info@chrisdaggas.com> - 1.0.0-1
- Initial release
