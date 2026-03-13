Name:           lnx-control-center
Version:        1.5.0
Release:        1%{?dist}
Summary:        Linux System Change & Activity Timeline Viewer

License:        MIT
URL:            https://chrisdaggas.com
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  pkg-config

Requires:       gtk4
Requires:       libadwaita

%description
Control Center is a modern GTK4/Libadwaita desktop application for monitoring
system health, managing services, viewing activity logs, and comparing
system state snapshots.

%prep
%autosetup

%build
cargo build --release

%install
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_datadir}/applications
mkdir -p %{buildroot}%{_datadir}/metainfo
mkdir -p %{buildroot}%{_datadir}/icons/hicolor/scalable/apps

install -m 755 target/release/control-center %{buildroot}%{_bindir}/control-center
install -m 644 data/com.chrisdaggas.control-center.desktop %{buildroot}%{_datadir}/applications/
install -m 644 data/com.chrisdaggas.control-center.metainfo.xml %{buildroot}%{_datadir}/metainfo/
install -m 644 data/icons/hicolor/scalable/apps/com.chrisdaggas.control-center.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/

%files
%{_bindir}/control-center
%{_datadir}/applications/com.chrisdaggas.control-center.desktop
%{_datadir}/metainfo/com.chrisdaggas.control-center.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/com.chrisdaggas.control-center.svg

%changelog
* Fri Mar 13 2026 Christos A. Daggas <info@chrisdaggas.com> - 1.5.0-1
- Add Security Posture page and security snapshot comparison support

* Mon Jun 30 2025 Christos A. Daggas <info@chrisdaggas.com> - 1.0.0-1
- Initial release
